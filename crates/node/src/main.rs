//! `quiddity-node`: a bare-bones command line for a graph on disk, and the
//! headless sync node.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode};
use quiddity_core::files::{export_dir, import_dir};
use quiddity_core::{Index, merge_duplicates};
use quiddity_node::store::Store;
use quiddity_node::{ALPN, Allowed, Peer};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// The graph's directory.
    #[arg(long, short, default_value = ".")]
    graph: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a graph. With --blank, an empty replica to sync an existing
    /// graph into.
    Init {
        #[arg(long)]
        blank: bool,
    },
    /// Import a directory of Markdown: a Quiddity export or a Logseq graph.
    Import { dir: PathBuf },
    /// Export the graph as a directory of Markdown.
    Export { dir: PathBuf },
    /// Print this device's ID, for other devices' --allow lists.
    Id,
    /// Serve the graph to other devices until interrupted.
    Serve {
        /// A device allowed to connect. Repeat for more.
        #[arg(long)]
        allow: Vec<EndpointId>,
        /// Allow any device to connect.
        #[arg(long, conflicts_with = "allow")]
        allow_any: bool,
        #[command(flatten)]
        net: Net,
    },
    /// Sync with another device until neither has anything more to send.
    Sync {
        peer: EndpointId,
        /// The peer's address, if it can't be found by its ID.
        #[arg(long)]
        addr: Option<SocketAddr>,
        #[command(flatten)]
        net: Net,
    },
}

#[derive(clap::Args)]
struct Net {
    /// Listen on this address only, without relays or address lookup:
    /// for a local network, or testing.
    #[arg(long)]
    local: Option<SocketAddr>,
}

impl Net {
    async fn endpoint(&self, store: &Store) -> anyhow::Result<Endpoint> {
        let key = store.key()?;
        let builder = match self.local {
            Some(addr) => Endpoint::builder(presets::Minimal)
                .relay_mode(RelayMode::Disabled)
                .bind_addr(addr)?,
            None => Endpoint::builder(presets::N0),
        };
        Ok(builder
            .secret_key(key)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await?)
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("quiddity-node: {e:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let store = Store::new(cli.graph);
    match cli.command {
        Command::Init { blank } => {
            store.init(blank)?;
            println!("{}", store.key()?.public());
        }
        Command::Import { dir } => {
            let graph = store.load()?;
            let imported = import_dir(&graph, &dir)?;
            merge_duplicates(&graph, &Index::build(&graph)?)?;
            store.save(&graph)?;
            println!("imported {} entities", imported.len());
        }
        Command::Export { dir } => {
            let written = export_dir(&store.load()?, &dir)?;
            println!("exported {written} entities");
        }
        Command::Id => println!("{}", store.key()?.public()),
        Command::Serve {
            allow,
            allow_any,
            net,
        } => {
            if allow.is_empty() && !allow_any {
                anyhow::bail!("say which devices may connect, with --allow or --allow-any");
            }
            let allowed = if allow_any {
                Allowed::Any
            } else {
                Allowed::Only(allow.into_iter().collect::<HashSet<_>>())
            };
            let peer = Peer::new(store.load()?, allowed);
            let endpoint = net.endpoint(&store).await?;
            match net.local {
                Some(_) => println!(
                    "serving {} at {}",
                    endpoint.id(),
                    endpoint.bound_sockets()[0]
                ),
                None => println!("serving {}", endpoint.id()),
            }
            let router = Router::builder(endpoint).accept(ALPN, peer.clone()).spawn();
            save_on_change(peer.clone(), store.clone());
            tokio::signal::ctrl_c().await?;
            router.shutdown().await?;
            store.save(peer.graph())?;
        }
        Command::Sync {
            peer: id,
            addr,
            net,
        } => {
            let peer = Peer::new(store.load()?, Allowed::Only(HashSet::from([id])));
            let endpoint = net.endpoint(&store).await?;
            let mut target = EndpointAddr::new(id);
            if let Some(addr) = addr {
                target = target.with_ip_addr(addr);
            }
            peer.connect(&endpoint, target, Some(Duration::from_secs(2)))
                .await?;
            let graph = peer.graph();
            merge_duplicates(graph, &Index::build(graph)?)?;
            store.save(graph)?;
            endpoint.close().await;
            println!("synced");
        }
    }
    Ok(())
}

/// Saves the graph shortly after it changes, batching bursts of changes.
/// Duplicate journals that arrived from different devices are merged first,
/// and the merge is sent on to connected devices.
fn save_on_change(peer: Peer, store: Store) {
    let mut changes = peer.subscribe();
    tokio::spawn(async move {
        while changes.changed().await.is_ok() {
            tokio::time::sleep(Duration::from_millis(500)).await;
            changes.mark_unchanged();
            let graph = peer.graph();
            let saved = (|| -> anyhow::Result<bool> {
                let moved = merge_duplicates(graph, &Index::build(graph)?)?;
                store.save(graph)?;
                Ok(moved > 0)
            })();
            match saved {
                Ok(true) => peer.changed(),
                Ok(false) => {}
                Err(e) => eprintln!("quiddity-node: saving failed: {e:#}"),
            }
        }
    });
}

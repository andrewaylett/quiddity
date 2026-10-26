import type { Metadata } from "next";
import { Suspense } from "react";
import "@fontsource/ibm-plex-sans/latin-400.css";
import "@fontsource/ibm-plex-sans/latin-400-italic.css";
import "@fontsource/ibm-plex-sans/latin-500.css";
import "@fontsource/ibm-plex-sans/latin-600.css";
import "./globals.css";
import { Skeleton } from "@/components/boundary";
import { LeftSidebar } from "@/components/left-sidebar";
import { RightPane } from "@/components/right-pane";
import { Shell } from "@/components/shell";

export const metadata: Metadata = { title: "Quiddity" };

/**
 * Both sidebars read the URL, which in a static export suspends up to the
 * nearest boundary. Each pane therefore has its own, so none can hold up
 * another; the right pane's fallback is empty so it reserves no space.
 */
export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en-GB">
      <body>
        <Shell
          left={
            <Suspense fallback={<Skeleton className="m-4 h-4 w-3/4" />}>
              <LeftSidebar />
            </Suspense>
          }
          right={
            <Suspense fallback={null}>
              <RightPane />
            </Suspense>
          }
        >
          <Suspense fallback={<Skeleton className="m-8 h-8 w-1/3" />}>{children}</Suspense>
        </Shell>
      </body>
    </html>
  );
}

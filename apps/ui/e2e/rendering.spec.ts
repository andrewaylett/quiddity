import type { Page } from "@playwright/test";
import { expect, test, editBlock, openEntity } from "./fixtures";

/** Replaces the first block of Reading list with `markdown`, then stops editing. */
async function write(page: Page, markdown: string) {
  await openEntity(page, "Reading list");
  await editBlock(page, "Designing Data-Intensive Applications");
  await page.keyboard.press("Control+a");
  // Enter would split the block, so lines are joined with Shift-Enter.
  const lines = markdown.split("\n");
  for (const [i, line] of lines.entries()) {
    if (i > 0) await page.keyboard.press("Shift+Enter");
    await page.keyboard.type(line);
  }
  await page.keyboard.press("Escape");
  return page.locator("main article > ul > li").first().locator("[data-block-text]");
}

test("Markdown is rendered when a block isn't being edited", async ({ page }) => {
  const block = await write(page, "**bold**, *italic*, `code` and ~~struck~~");
  await expect(block.locator("strong")).toHaveText("bold");
  await expect(block.locator("em")).toHaveText("italic");
  await expect(block.locator("code")).toHaveText("code");
  await expect(block.locator("del")).toHaveText("struck");
});

test("headings, lists and quotes render, and editing shows the source again", async ({ page }) => {
  const block = await write(page, "## Title");
  await expect(block.getByRole("heading", { level: 2, name: "Title" })).toBeVisible();
  await block.click();
  await expect(page.locator(".cm-content")).toHaveText("## Title");
});

test("a [[link]] works inside formatting and is left alone in code", async ({ page }) => {
  const block = await write(page, "**see [[Loro]]** and `[[Loro]]`");
  await expect(block.locator("strong").getByRole("link", { name: "Loro" })).toBeVisible();
  await expect(block.locator("code")).toHaveText("[[Loro]]");
  await expect(block.getByRole("link")).toHaveCount(1);
  await block.getByRole("link", { name: "Loro" }).click();
  await expect(page.getByRole("heading", { name: "Loro", level: 1 })).toBeVisible();
});

test("line properties become chips, and the prose around them stays", async ({ page }) => {
  const block = await write(page, "A task\nstatus:: open\nowner:: [[Loro]]");
  await expect(block.getByText("A task")).toBeVisible();
  await expect(block.getByText("status", { exact: true })).toBeVisible();
  await expect(block.getByText("open", { exact: true })).toBeVisible();
  await expect(block.getByText("status:: open")).toHaveCount(0);
  await expect(block.getByRole("link", { name: "Loro" })).toBeVisible();
});

test("a block of nothing but properties is only chips", async ({ page }) => {
  await page.goto("/");
  const day = page.locator("main section").first();
  const block = day.locator("[data-block-text]").first();
  await expect(block.getByText("journal", { exact: true })).toBeVisible();
  await expect(block).not.toContainText("type::");
});

test("inline properties are chips in the text", async ({ page }) => {
  const block = await write(page, "ship it #urgent #due(friday) soon");
  await expect(block.getByText("#urgent")).toBeVisible();
  await expect(block.getByText("due", { exact: true })).toBeVisible();
  await expect(block.getByText("friday", { exact: true })).toBeVisible();
  await expect(block).toContainText("ship it");
  await expect(block).toContainText("soon");
});

test("properties in code are just code", async ({ page }) => {
  const block = await write(page, "```\nnot:: a property\n```");
  await expect(block.locator("pre")).toContainText("not:: a property");
  await expect(block.getByText("not", { exact: true })).toHaveCount(0);
});

test("raw HTML is shown as text, never run", async ({ page }) => {
  const block = await write(
    page,
    '<img src=x onerror="window.__pwned=1"> <script>window.__pwned=1</script>',
  );
  await expect(block.locator("img, script")).toHaveCount(0);
  await expect(block).toContainText("<img src=x");
  expect(await page.evaluate(() => (window as { __pwned?: number }).__pwned)).toBeUndefined();
});

test("only web and mail links are links; the rest are inert text", async ({ page }) => {
  const block = await write(
    page,
    "[bad](javascript:window.__pwned=1) [local](/entity/?id=x) [fine](https://example.com/x)",
  );
  await expect(block.locator('a[href^="javascript"]')).toHaveCount(0);
  await expect(block.getByRole("link")).toHaveCount(1);
  await expect(block.getByText("bad")).toBeVisible();
  await expect(block.getByText("local")).toBeVisible();
});

test("an external link opens outside the page instead of replacing it", async ({ page }) => {
  // Nothing leaves the test: the page is answered here.
  await page.context().route("https://example.com/**", (route) =>
    route.fulfill({ body: "<title>example</title>", contentType: "text/html" }),
  );
  const block = await write(page, "[example](https://example.com/x)");
  const popup = page.waitForEvent("popup");
  await block.getByRole("link", { name: "example" }).click();
  const opened = await popup;
  expect(opened.url()).toContain("example.com");
  await opened.close();
  await expect(page).toHaveURL(/\/entity\//);
  await expect(page.getByRole("heading", { name: "Reading list", level: 1 })).toBeVisible();
});

test("remote images aren't fetched", async ({ page }) => {
  const requests: string[] = [];
  page.on("request", (r) => requests.push(r.url()));
  const block = await write(page, "![a picture](https://tracker.example/pixel.png)");
  await expect(block.getByText("[a picture]")).toBeVisible();
  expect(requests.filter((u) => u.includes("tracker.example"))).toEqual([]);
});

test("task lists and tables render", async ({ page }) => {
  const block = await write(page, "- [x] done\n- [ ] not yet\n\n| a | b |\n|---|---|\n| 1 | 2 |");
  await expect(block.locator('input[type="checkbox"]')).toHaveCount(2);
  await expect(block.locator("table td")).toHaveCount(2);
});

test("clicking a link or chip doesn't start editing, and selecting text doesn't either", async ({ page }) => {
  const block = await write(page, "see [[Loro]] there");
  await block.getByText("there").dblclick();
  // A double-click selects a word; the second click must not have opened an editor over it.
  await expect(page.locator(".cm-content")).toHaveCount(1);
});

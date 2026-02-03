import { test, expect } from "bun:test";
import { renderHtml, renderMarkdown } from "./render";

test("renderHtml includes title", () => {
  const output = renderHtml("My Guide");
  expect(output).toContain("My Guide");
});

test("renderMarkdown includes title", () => {
  const output = renderMarkdown("My Guide");
  expect(output).toContain("My Guide");
});

test("renderMarkdown escapes < > &", () => {
  const output = renderMarkdown("A & B <C>");
  expect(output).toContain("A &amp; B &lt;C&gt;");
});

test("renderHtml escapes < > &", () => {
  const output = renderHtml("A & B <C>");
  expect(output).toContain("A &amp; B &lt;C&gt;");
});

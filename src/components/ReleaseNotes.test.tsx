import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import ReleaseNotes from "./ReleaseNotes";

describe("ReleaseNotes", () => {
  it("renders bullet list items", () => {
    render(<ReleaseNotes body={"- First change\n- Second change"} />);
    expect(screen.getByText("First change")).toBeInTheDocument();
    expect(screen.getByText("Second change")).toBeInTheDocument();
    expect(document.querySelectorAll("li")).toHaveLength(2);
  });

  it("strips PR references and GitHub links", () => {
    const body = "- Added export feature by @dev in https://github.com/w0nk1/StepCast/pull/12";
    render(<ReleaseNotes body={body} />);
    expect(screen.getByText("Added export feature")).toBeInTheDocument();
    expect(screen.queryByText(/@dev/)).not.toBeInTheDocument();
  });

  it("strips inline PR refs like (#123)", () => {
    render(<ReleaseNotes body={"- Fix crash on export (#42)"} />);
    expect(screen.getByText("Fix crash on export")).toBeInTheDocument();
  });

  it("skips markdown headings", () => {
    render(<ReleaseNotes body={"## What's Changed\n- Better export"} />);
    expect(screen.queryByText(/What's Changed/)).not.toBeInTheDocument();
    expect(screen.getByText("Better export")).toBeInTheDocument();
  });

  it("renders bold text", () => {
    render(<ReleaseNotes body={"- **New:** WebP export support"} />);
    const strong = document.querySelector("strong");
    expect(strong).toHaveTextContent("New:");
  });

  it("renders plain paragraphs", () => {
    render(<ReleaseNotes body={"A simple update with improvements."} />);
    expect(screen.getByText("A simple update with improvements.")).toBeInTheDocument();
  });

  it("returns null for empty body", () => {
    const { container } = render(<ReleaseNotes body={""} />);
    expect(container.innerHTML).toBe("");
  });
});

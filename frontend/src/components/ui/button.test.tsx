import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Button } from "./button";

describe("shadcn Button", () => {
  it("renders its children", () => {
    render(<Button>Create token</Button>);
    expect(
      screen.getByRole("button", { name: "Create token" })
    ).toBeInTheDocument();
  });

  it("applies the primary (brand) background by default", () => {
    render(<Button>Go</Button>);
    expect(screen.getByRole("button", { name: "Go" })).toHaveClass(
      "bg-primary"
    );
  });

  it("fires onClick", async () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Click</Button>);
    await userEvent.click(screen.getByRole("button", { name: "Click" }));
    expect(onClick).toHaveBeenCalledOnce();
  });

  it("renders as a child element when asChild is set", () => {
    render(
      <Button asChild>
        <a href="/x">Link button</a>
      </Button>
    );
    const link = screen.getByRole("link", { name: "Link button" });
    expect(link).toHaveAttribute("href", "/x");
  });
});

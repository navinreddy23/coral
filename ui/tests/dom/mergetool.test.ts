// @vitest-environment happy-dom
import { render } from "@testing-library/svelte";
import { fireEvent } from "@testing-library/dom";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../../src/ipc/invoke", () => ({
  invoke: vi.fn(),
  isPreview: () => false,
}));
// The window subscribes to terminal output and to repository changes. Neither channel
// exists without the Tauri shell, and the real `listen` throws rather than returning.
vi.mock("@tauri-apps/api/event", () => ({
  listen: async () => () => undefined,
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

import MergeTool from "../../src/app/MergeTool.svelte";
import { MergeState } from "../../src/state/merge.svelte";
import type { ConflictedFile, Operation } from "../../src/ipc/types";

function operation(swapped = false): Operation {
  return {
    state: swapped ? "rebase" : "merge",
    labels: swapped
      ? { ours: "main", theirs: "feature", swapped: true }
      : { ours: "main", theirs: "side", swapped: false },
    progress: null,
    headName: null,
    stoppedAt: null,
    interactive: false,
    resumable: true,
    applying: false,
    prepared: null,
  };
}

function conflicted(over: Partial<ConflictedFile> = {}): ConflictedFile {
  return {
    path: "f.txt",
    kind: "both_modified",
    binary: false,
    deleteModify: false,
    lfs: false,
    ...over,
  };
}

function state(files: ConflictedFile[], swapped = false) {
  const merge = new MergeState();
  merge.operation = operation(swapped);
  merge.files = files;
  return merge;
}

const noop = () => {};

describe("the merge tool", () => {
  it("will not continue while a file is still conflicted", () => {
    const { container } = render(MergeTool, {
      props: { merge: state([conflicted()]), onDone: noop },
    });
    const buttons = [...container.querySelectorAll("header button")];
    const cont = buttons.find((b) => b.textContent?.trim() === "Continue");
    expect(cont).toBeDefined();
    expect((cont as HTMLButtonElement).disabled).toBe(true);
  });

  it("offers neither continue nor abort when git has no operation to continue", () => {
    // A stash that would not apply, or a merge asked not to commit: the index is conflicted
    // and there is no marker file. Both buttons answered "no merge, rebase, cherry-pick or
    // revert is in progress", which is true and no help at all.
    const merge = state([conflicted()]);
    if (merge.operation)
      merge.operation = { ...merge.operation, resumable: false };
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });

    const labels = [...container.querySelectorAll("header button")].map((b) =>
      b.textContent?.trim(),
    );
    expect(labels).not.toContain("Continue");
    expect(labels).not.toContain("Abort");
    expect(container.querySelector("header")?.textContent).toContain(
      "conflicts to resolve",
    );
    expect(container.querySelector("header")?.textContent).not.toContain(
      "in progress",
    );
    expect(container.querySelector("header")?.textContent).toContain(
      "then commit as usual",
    );
  });

  it("lets the operation finish once nothing is left", () => {
    const { container } = render(MergeTool, {
      props: { merge: state([]), onDone: noop },
    });
    const cont = [...container.querySelectorAll("header button")].find(
      (b) => b.textContent?.trim() === "Continue",
    );
    expect((cont as HTMLButtonElement).disabled).toBe(false);
    expect(container.textContent).toContain("Every file is resolved");
  });

  it('names the sides after the branches, not "ours" and "theirs"', () => {
    const { container } = render(MergeTool, {
      props: { merge: state([conflicted()]), onDone: noop },
    });
    const wholesale = container.querySelector(".wholesale")?.textContent ?? "";
    expect(wholesale).toContain("main");
    expect(wholesale).toContain("side");
  });

  it("warns that a rebase reverses the sides", () => {
    const { container } = render(MergeTool, {
      props: { merge: state([conflicted()], true), onDone: noop },
    });
    expect(container.querySelector(".warn")?.textContent).toContain("reversed");
  });

  it("offers only whole-file choices for a binary conflict", async () => {
    const merge = state([conflicted({ binary: true })]);
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    const file = container.querySelector("button.file") as HTMLButtonElement;
    expect(file.textContent).toContain("whole file");

    // Selectable all the same: there is still a choice to make, and disabling it left the
    // pane telling people to pick regions in a file that has none.
    expect(file.disabled).toBe(false);
    await fireEvent.click(file);
    expect(container.querySelector(".whole")?.textContent).toContain("binary");
    expect(container.querySelector(".conflict")).toBeNull();
    // Both sides have one; deleting it is not one of the two answers.
    const choices = [...container.querySelectorAll(".choices button")].map(
      (b) => b.textContent?.trim(),
    );
    expect(choices).toEqual([
      "Keep what is on main",
      "Take the version from side",
    ]);
  });

  it("offers only whole-file choices for a file kept behind a filter", async () => {
    // Git LFS stores a pointer of three lines and keeps the asset outside the repository.
    // Offered as text, the pane invited a resolution taking one side's object and the other's
    // size, which names nothing: the commit went out and every clone after it had no file.
    const merge = state([conflicted({ path: "logo.png", lfs: true })]);
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    const file = container.querySelector("button.file") as HTMLButtonElement;
    expect(file.textContent).toContain("whole file");

    await fireEvent.click(file);
    expect(container.querySelector(".whole")?.textContent).toContain("Git LFS");
    expect(container.querySelector(".conflict")).toBeNull();
  });

  it("explains a file deleted on this side and changed by the commit, and offers the two ways out", async () => {
    // The reported case: cherry-picking a commit that changes a file this branch does not
    // have. There is one version of it, so a region picker has nothing to pick between.
    const merge = state([
      conflicted({ kind: "deleted_by_us", deleteModify: true }),
    ]);
    merge.operation = {
      state: "cherry_pick",
      labels: {
        ours: "dummyx",
        theirs: "3755eae (test conflicts)",
        swapped: false,
      },
      progress: null,
      headName: null,
      stoppedAt: null,
      interactive: false,
      resumable: true,
      applying: false,
      prepared: null,
    };
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });

    expect(container.querySelector("header")?.textContent).toContain(
      "cherry-pick in progress",
    );
    // The side that does not exist is not offered; taking it could only ever fail.
    const wholesale = container.querySelector(".wholesale")?.textContent ?? "";
    expect(wholesale).not.toContain("dummyx");
    expect(wholesale).toContain("3755eae");

    await fireEvent.click(
      container.querySelector("button.file") as HTMLButtonElement,
    );
    const said = container.querySelector(".whole")?.textContent ?? "";
    expect(said).toContain("is not on dummyx");
    const choices = [...container.querySelectorAll(".choices button")].map(
      (b) => b.textContent?.trim(),
    );
    expect(choices).toEqual([
      "Take the version from 3755eae (test conflicts)",
      "Leave it deleted",
    ]);
  });

  it("offers a delete for a file removed on one side", () => {
    const merge = state([conflicted({ deleteModify: true })]);
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    expect(container.querySelector(".wholesale")?.textContent).toContain(
      "delete",
    );
  });

  it("draws each side as the whole file, and says how many regions are untouched", () => {
    const merge = state([conflicted()]);
    merge.active = "f.txt";
    merge.blocks = {
      blocks: [
        { kind: "common", lines: ["one"] },
        { kind: "conflict", base: ["two"], ours: ["MAIN"], theirs: ["SIDE"] },
        { kind: "common", lines: ["three"] },
      ],
    };
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });

    const [ours, theirs] = [
      ...container.querySelectorAll(".pane"),
    ] as HTMLElement[];
    expect(ours?.textContent).toContain("one");
    expect(ours?.textContent).toContain("MAIN");
    expect(ours?.textContent).toContain("three");
    expect(theirs?.textContent).toContain("SIDE");
    expect(theirs?.textContent).not.toContain("MAIN");

    // Untouched means the region is still a question, which is worth saying out loud — and
    // the result shows it as one rather than picking the base to stand in for an answer.
    expect(container.querySelector(".bar")?.textContent).toContain(
      "1 untouched",
    );
    const result = container.querySelector(".result")?.textContent ?? "";
    expect(result).toContain("MAIN");
    expect(result).toContain("SIDE");
    expect(result, "the base is not an answer").not.toContain("two");
  });

  it("says which commit failed to apply when a step stops on the next one", () => {
    // A rebase stops once per conflicting commit. Continuing usually lands on the next one,
    // and without this the window looked identical to having finished: the file list refills
    // and nothing says why.
    const merge = state([conflicted()], true);
    merge.stopped = "error: could not apply 91e605d... local: dummy1 file";
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    expect(container.textContent).toContain("could not apply 91e605d");
  });

  it("says nothing of the kind before a step has stopped", () => {
    const { container } = render(MergeTool, {
      props: { merge: state([conflicted()]), onDone: noop },
    });
    expect(container.querySelector(".stopped")).toBeNull();
  });
});

/**
 * Picking region by region, which is how a conflict is actually settled.
 *
 * The sides are toggles, not one choice of three: a great many conflicts are resolved by
 * keeping both lines, and the order they are taken in is part of the answer.
 */
describe("picking in the merge tool", () => {
  /** One file open, one conflict, two lines on each side of it. */
  function opened(swapped = false) {
    const merge = state([conflicted()], swapped);
    merge.active = "f.txt";
    merge.blocks = {
      blocks: [
        { kind: "common", lines: ["one"] },
        {
          kind: "conflict",
          base: ["was"],
          ours: ["MAIN a", "MAIN b"],
          theirs: ["SIDE a", "SIDE b"],
        },
        { kind: "common", lines: ["last"] },
      ],
    };
    return merge;
  }

  /** The checkboxes of one pane, in file order. */
  function ticks(
    container: HTMLElement,
    side: "ours" | "theirs",
  ): HTMLInputElement[] {
    return [
      ...container.querySelectorAll<HTMLInputElement>(
        `.pane.${side} input[type="checkbox"]`,
      ),
    ];
  }

  function resultText(container: HTMLElement): string {
    return [...container.querySelectorAll(".result .line .text")]
      .map((e) => e.textContent)
      .join("\n");
  }

  it("puts a checkbox on every conflicting line and none on the agreed ones", () => {
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    // Two lines each side, and nothing on `one` or `last`.
    expect(ticks(container, "ours")).toHaveLength(2);
    expect(ticks(container, "theirs")).toHaveLength(2);
    expect(container.querySelectorAll(".pane.ours .line")).toHaveLength(4);
  });

  it("takes one line from each side, in the order they were clicked", async () => {
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });

    await fireEvent.click(ticks(container, "theirs")[1] as HTMLInputElement);
    await fireEvent.click(ticks(container, "ours")[0] as HTMLInputElement);

    expect(merge.choices[0]).toEqual([
      { side: "theirs", line: 1 },
      { side: "ours", line: 0 },
    ]);
    expect(resultText(container)).toBe("one\nSIDE b\nMAIN a\nlast");
  });

  it("takes a line back out when its box is cleared", async () => {
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    await fireEvent.click(ticks(container, "ours")[0] as HTMLInputElement);
    await fireEvent.click(ticks(container, "ours")[1] as HTMLInputElement);
    expect(resultText(container)).toBe("one\nMAIN a\nMAIN b\nlast");

    await fireEvent.click(ticks(container, "ours")[0] as HTMLInputElement);
    expect(resultText(container)).toBe("one\nMAIN b\nlast");
  });

  it("shows an unanswered region as markers rather than as the base", () => {
    // It used to show the base, and Mark resolved wrote what it showed: a merge could be
    // finished with the lines from before either branch touched them, which is neither side's
    // work and looks like a resolution. Now the pane shows what the file would be — a question
    // — and the button that writes it refuses.
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    expect(resultText(container)).toBe(
      "one\n<<<<<<< main\nMAIN a\nMAIN b\n=======\nSIDE a\nSIDE b\n>>>>>>> side\nlast",
    );
  });

  it("will not mark a file resolved while a region is unanswered", () => {
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    const mark = [...container.querySelectorAll("button")].find(
      (b) => b.textContent?.trim() === "Mark resolved",
    ) as HTMLButtonElement;
    expect(mark.disabled, "nothing has been taken yet").toBe(true);
    expect(mark.title).toContain("still needs a side taken");
  });

  it("lets a file be marked resolved once every region has an answer", async () => {
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    await fireEvent.click(ticks(container, "theirs")[0] as HTMLInputElement);
    const mark = [...container.querySelectorAll("button")].find(
      (b) => b.textContent?.trim() === "Mark resolved",
    ) as HTMLButtonElement;
    expect(mark.disabled).toBe(false);
    expect(resultText(container)).toBe("one\nSIDE a\nlast");
  });

  it("counts a region emptied on purpose as answered", async () => {
    // Ticking a line and unticking it again says "keep neither side here", which is a decision
    // and has to be distinguishable from never having looked at the region.
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    await fireEvent.click(ticks(container, "ours")[0] as HTMLInputElement);
    await fireEvent.click(ticks(container, "ours")[0] as HTMLInputElement);
    expect(merge.settled, "answered, even though it takes nothing").toBe(true);
    expect(resultText(container)).toBe("one\n(nothing taken)\nlast");
  });

  it("carries the whole of a take-all label on hover, since the button caps its width", async () => {
    // "All the incoming change" came out as "All the incoming cha…", which is a control whose
    // label ends mid-word. The cap has to stay, because a branch name has no length limit, so
    // whatever it cuts is reachable.
    const merge = state([conflicted()]);
    merge.active = "f.txt";
    merge.blocks = {
      blocks: [{ kind: "conflict", base: ["w"], ours: ["A"], theirs: ["B"] }],
    };
    merge.operation = {
      state: "cherry_pick",
      labels: { ours: "feature/good-name", theirs: "the incoming change", swapped: false },
      progress: null,
      headName: null,
      stoppedAt: null,
      interactive: false,
      resumable: true,
      applying: false,
      prepared: null,
    };
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });

    const titles = [...container.querySelectorAll("button")]
      .filter((b) => (b.textContent ?? "").trim().startsWith("All "))
      .map((b) => b.getAttribute("title"));
    expect(titles).toContain("All the incoming change");
    expect(titles).toContain("All feature/good-name");
  });

  it("takes a whole side of one conflict from its pane heading", async () => {
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    const takeAll = [
      ...container.querySelectorAll(".pane.theirs .head button"),
    ][0];
    await fireEvent.click(takeAll as HTMLButtonElement);

    expect(ticks(container, "theirs").every((t) => t.checked)).toBe(true);
    expect(resultText(container)).toBe("one\nSIDE a\nSIDE b\nlast");
  });

  it("steps between conflicts and says which one it is on", async () => {
    const merge = state([conflicted()]);
    merge.active = "f.txt";
    merge.blocks = {
      blocks: [
        { kind: "conflict", base: ["w1"], ours: ["A1"], theirs: ["B1"] },
        { kind: "common", lines: ["between"] },
        { kind: "conflict", base: ["w2"], ours: ["A2"], theirs: ["B2"] },
      ],
    };
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    const bar = () =>
      container.querySelector(".output .bar")?.textContent ?? "";
    expect(bar()).toContain("conflict 1 of 2");

    const next = container.querySelector(
      '[aria-label="Next conflict"]',
    ) as HTMLButtonElement;
    await fireEvent.click(next);
    expect(bar()).toContain("conflict 2 of 2");
    // And it wraps, rather than stopping at the end with no way back round.
    await fireEvent.click(next);
    expect(bar()).toContain("conflict 1 of 2");
  });

  it("lets the result be typed over, and the picks be gone back to", async () => {
    const merge = opened();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    await fireEvent.click(ticks(container, "ours")[0] as HTMLInputElement);

    const edit = [...container.querySelectorAll("button")].find(
      (b) => b.textContent?.trim() === "Edit it by hand",
    ) as HTMLButtonElement;
    await fireEvent.click(edit);

    const box = container.querySelector(
      "textarea.typed",
    ) as HTMLTextAreaElement;
    expect(box.value).toBe("one\nMAIN a\nlast\n");
    await fireEvent.input(box, {
      target: { value: "one\nsomething else\nlast\n" },
    });
    expect(merge.output).toBe("one\nsomething else\nlast\n");
    // The picks no longer decide the file, so they are not live either.
    expect((ticks(container, "ours")[0] as HTMLInputElement).disabled).toBe(
      true,
    );

    const back = [...container.querySelectorAll("button")].find(
      (b) => b.textContent?.trim() === "Back to picking sides",
    ) as HTMLButtonElement;
    await fireEvent.click(back);
    expect(merge.output).toBe("one\nMAIN a\nlast\n");
  });

  it("offers to skip a commit during a rebase, and not during a merge", () => {
    const rebase = render(MergeTool, {
      props: { merge: opened(true), onDone: noop },
    });
    const labels = [...rebase.container.querySelectorAll("header button")].map(
      (b) => b.textContent?.trim(),
    );
    expect(labels).toContain("Skip commit");

    // `git merge --skip` does not exist: there is one commit being made, and skipping it is
    // aborting.
    const merge = render(MergeTool, {
      props: { merge: opened(false), onDone: noop },
    });
    const during = [...merge.container.querySelectorAll("header button")].map(
      (b) => b.textContent?.trim(),
    );
    expect(during).not.toContain("Skip commit");
  });
});

describe("a very large conflicted file", () => {
  /** Thirty thousand lines, which is the size of the kernel's MAINTAINERS. */
  function huge() {
    const merge = state([conflicted()]);
    merge.active = "MAINTAINERS";
    const before = Array.from({ length: 15000 }, (_, i) => `line ${i}`);
    const after = Array.from({ length: 15000 }, (_, i) => `tail ${i}`);
    merge.blocks = {
      blocks: [
        { kind: "common", lines: before },
        { kind: "conflict", base: ["was"], ours: ["MAIN"], theirs: ["SIDE"] },
        { kind: "common", lines: after },
      ],
    };
    return merge;
  }

  it("builds only the lines on screen, not the whole file three times over", () => {
    // Every line of every pane was in the page: a quarter of a million elements for this
    // file, and the window sat on "Loading…" for minutes.
    const merge = huge();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });

    const lines = container.querySelectorAll(".line");
    expect(lines.length).toBeGreaterThan(0);
    expect(lines.length).toBeLessThan(1000);
  });

  it("still says how long the file is, so the scrollbar tells the truth", () => {
    const merge = huge();
    const { container } = render(MergeTool, { props: { merge, onDone: noop } });
    const spacer = container.querySelector(".code .spacer") as HTMLElement;
    // 30,001 lines at seventeen pixels each.
    expect(spacer.style.height).toBe(`${30001 * 17}px`);
  });
});

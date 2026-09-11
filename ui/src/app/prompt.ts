/**
 * Asking for a name that git has the last word on.
 *
 * git is the authority on what a ref may be called, and it only says so once the command runs.
 * Every one of these dialogs closed before that happened, so a name git would not take was
 * typed again from nothing, with the reason in a toast behind the dialog that had just gone.
 */

/**
 * Asks for a line of text and does something with it, asking again while the attempt is refused.
 *
 * Returns when the work was accepted, when the box came back empty, or when the user cancelled.
 * The text that was refused comes back in the box, since correcting a name is most of why
 * anybody is looking at this dialog a second time.
 */
export async function askUntilAccepted(
  initial: string,
  ask: (initial: string) => Promise<string | null>,
  run: (text: string) => Promise<boolean>,
): Promise<void> {
  let again = initial;
  for (;;) {
    const got = await ask(again);
    if (got === null || got.trim() === '') return;
    if (await run(got.trim())) return;
    again = got;
  }
}

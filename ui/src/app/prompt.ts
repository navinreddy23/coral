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
 * `run` answers whether the dialog is finished with, which is not the same as whether the work
 * succeeded: see {@link nameWasRefused}. Returns when it is finished with, when the box came
 * back empty, or when the user cancelled. The text that was refused comes back in the box,
 * since correcting a name is most of why anybody is looking at this dialog a second time.
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

/**
 * Whether git refused the name itself.
 *
 * The only failure a second name fixes. A create-and-switch stopped by local changes, a push
 * the remote would not take: asking for the name again, in the same box with the same text
 * still in it, says the name was the problem and invites a user to keep trying names that
 * cannot work. Sound to read because the locale is pinned to C wherever Coral runs git.
 */
export function nameWasRefused(message: string | undefined | null): boolean {
  if (message === undefined || message === null) return false;
  return /is not a valid (?:branch|tag) name|already exists/iu.test(message);
}

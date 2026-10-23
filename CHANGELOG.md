# Changelog

## Unreleased

### Ignore a file from the staging panel

An untracked file's menu has an Ignore submenu: the file itself, or every file with its
extension. A wholly untracked folder can be ignored as a folder. Each choice shows the line it
adds to the repository's `.gitignore`, which is only ever appended to. A file's own folder is
not offered, since ignoring a folder that holds tracked files would quietly ignore every new
file put beside them.

### Signatures in the commit details

A signed commit's details say what git makes of its signature: good, from whom, and whether
the key is trusted; bad; expired; revoked; or signed but impossible to check here, with git's
reason, such as an SSH signature with no allowed signers file. git's own verdict letter calls
that last case "no signature", and a signing program that cannot run "bad"; Coral reads git's
message beside the letter so neither is misreported.

### Commit templates

A repository's `commit.template` is where the message box starts, and where it returns after
each commit. As at git's own editor, a template left exactly as it was is not a message: the
commit button asks for it to be filled in.

## 1.2.7

### Checkboxes that fit the theme

Checkboxes are drawn in the window's own colours. In the dark theme an unticked one was a
bright white square, and a ticked one took the desktop's orange rather than the window's
accent.

### Starting out, smoothed

- The clone and create forms, and the form for adding a remote, put the cursor in their first
  empty field and submit on Enter. A repository's first remote is filled in as `origin`.
- "Add remote" waits for a URL as well as a name.
- Fetch, pull and push are off, with a tooltip saying why, until the repository has a remote.
  Fetching used to do nothing and say nothing, and pushing answered with git's "No configured
  push destination".
- A name git refuses, for a branch or a tag, is asked for again with git's reason in the
  dialog. The reason used to be in a notice hidden behind it.
- Making a working tree suggests a new folder beside the repository, which git creates. It
  opened a folder picker at the repository's parent, which git then refused for not being
  empty. The branch can be left empty for a detached checkout, as the dialog says.
- A new working tree appears in the side panel at once, and removing one closes its tab and
  takes away the failure notice from the first attempt once it is gone. Removal asks about
  throwing changes away only when changes are what stopped it.
- Continuing or skipping in a merge, rebase or cherry-pick asks who you are when git has not
  been told, as committing already did, and then carries on. It stopped on git's "unable to
  auto-detect email address".
- During a rebase, the two sides of a conflict say what they hold: the commit being replayed,
  and, past the first commit, that the target already carries the ones replayed before it.
  The buttons that take a whole side use the pane's letter when its name is too long.
- In a repository without a commit-graph, the graph column no longer keeps the width of the
  quick first screen once the real one replaces it, which could leave the messages far from
  the lanes.
- Menu items no longer show a tooltip repeating their own label, which covered the item
  beneath. One appears only when an item's text is cut short.
- Deleting a tag from a remote that never had it says so, rather than asking to confirm a
  deletion git would refuse.
- Right-clicking a branch or tag label in the graph opens that ref's own menu, with push and
  delete for a tag. It opened the commit's, which had neither.
- Stash is off until the first commit, and says why. It used to fail with "git stash exited
  with 1" and nothing after it, because the one line git gives was silenced.

### Menus from the keyboard

The Menu key, or Shift+F10, opens the selected commit's menu, and every menu can be worked
with the keys: the arrows move through it, Right and Left open and close a submenu, Enter
chooses and Escape closes. On a file or a branch that has the focus, the key opens that
item's menu instead. A commit's actions were reachable only with the mouse, and the arrow keys
moved the graph's selection beneath an open menu.

### Two remotes

A remote branch's menu can make the local branch of the same name track it instead: "Make main
track upstream/main". Pull and push follow it from then on. That menu no longer offers to
check out `main` when `main` is the branch already out, which did nothing while claiming it
would track the other remote.

### Small things found in a day's work

- Double-clicking a branch in the side panel checks it out, as double-clicking its label in
  the graph already did. It only selected it.
- After "Pull and rebase" from a refused push, the notice saying the push was refused goes
  away. It stayed up beside the pull that had dealt with it.
- Dropping a stash no longer claims its changes are nowhere else. After a pop that stopped on
  conflicts the stash is kept and its changes are in the working copy too.
- The staging panel's header stays on one line with a long branch name, which is shortened
  instead. The discard button used to drop onto a line of its own.
- Staging a whole file moves its diff to the staged side, and discarding it closes the diff.
  The panel stayed on a side the file had left, saying it had no changes there.
- Choosing SSH, or another program, on the commit signing page lists the keys it can use at
  once. The list stayed on the saved format's keys until Save, so SSH keys looked missing.
- The checkboxes on the commit signing and SSH pages, and the draft box on a new pull request,
  are drawn like every other checkbox. They were empty bars without a tick.

### The palette finds the command you named

A command whose name contains what you typed now comes before one that only has the same
letters scattered through it. Typing "theme" offered "Move the terminal to the right" ahead of
the theme commands.

## 1.2.6

### Fixes

- With uncommitted work, End and the mouse wheel stopped a row short of the oldest commit,
  which could then only be reached by dragging the scrollbar. The pinned WIP row is now
  taken out of the space the rows have.
- A short history with uncommitted work could scroll a few pixels for no reason, which the
  graph's steady scrollbar showed as a bar.

### Settings and the theme under one gear

The gear in the title bar now opens a menu: **Settings…**, then **Light theme**, **Dark theme**
and **Follow the desktop**, with the one in force ticked. The separate sun-and-moon button
beside it is gone; two glyphs of the same size side by side were easy to mistake for each
other, and neither said what it did.

### A large repository's first screen is its final one

With a commit-graph, the first screen is drawn in the order the full graph will have, and it
already knows how many commits there are. Before, it was drawn by commit time and rearranged
a few seconds later, when the full walk landed. On the Linux kernel the rows, lanes and
messages are final within two seconds, nothing on the graph moves when the full walk lands,
and that walk is ready in about 3.5 s instead of 6. The graph's scrollbar now stays put
rather than appearing and fading on its own a few seconds after opening. Commit
messages on the first screen no longer wait for the full walk, and the working tree's status
is read alongside the walk rather than after it.

### Sharper authors' pictures

Pictures are fetched at twice the size, so they are no longer enlarged on a high-density
screen, and they are shrunk to the node in halving steps rather than in one, which left them
jagged.

### Recover lost work

**Recover lost work** in the command palette lists every commit HEAD has been on, newest
first, from git's reflog. The ones no branch, tag or stash reaches any more are marked
**lost**: these are what a hard reset, a rebase or a deleted branch left behind. Each row has
**Branch here…** to keep the commit without leaving the branch you are on, or **Check out**
to look at it first. **Only lost**
hides the rest.

### Check out a pull or merge request

Right-click a request in the sidebar and choose **Check out #N locally**. Coral fetches the
request's tip by the name the host keeps it under, so a request from a fork works as well as
one from a branch here. It then checks it out as `pr/N`, or `mr/N` on GitLab. Doing it again
after the request has moved on brings the branch up to date. A branch that would lose commits
is left alone and the reason is shown, because that means the request was force-pushed or
something was committed to it here.

## 1.2.5

### Authors' pictures

Commits in the graph and the details panel show their author's picture in place of their
initials, where one can be found. A GitHub noreply address is looked up on GitHub; any other
address is looked up on Gravatar, which is sent a SHA-256 hash of it and nothing else. Each
author is asked about once and kept on disk, and an author with no picture keeps their
initials. **Appearance → Authors** turns it off.

## 1.2.4

### Lanes read as ribbons

Each row's lane colour now starts at its commit's node and runs across the message, with the
lines drawn over it, as GitKraken does. Before, it began after whichever lane line ran furthest
right on that row and stopped at a white message cell, so its edge jumped about and looked cut
off. The selected row is shown in a much deeper shade of its own lane.

### Find commits by what they changed, and when

The find box (`Ctrl+F`) reads three qualifiers beside the words it already matched:

- **`code:render_row`** finds the commits that added or removed the text, whatever their
  messages say. **`code:"/fn [a-z_]+_row/"`** takes a regular expression instead.
- **`after:2024-05-01`** and **`before:2.weeks`** bound the search by date. A rebased commit
  carrying an old date does not hide the newer ones beneath it.

A qualifier on its own lists every commit it lets through. A date Coral cannot read is said
under the field, rather than looking like a search that found nothing.

Typing a new search now stops the one still running, and closing the box stops it too. Before,
a phrase typed with pauses in it on a large repository queued a walk of the whole history per
pause.

### Bisect

Find the commit that broke something without leaving the window. Right-click a commit and
choose **Bisect: this commit is bad**, then mark one that was good; Coral checks out the commit
halfway between, and a strip over the list asks whether it is **Good** or **Bad** (or **Skip**,
for one that will not build). Each verdict narrows the suspects until the strip names the first
bad commit. **End bisect** puts you back where you started.

Every judged commit is tagged in the list. A bisect started in a terminal is picked up and can
be carried on here, in its own terms if it was started with `--term-new`/`--term-old`.

- **A bisect in progress no longer hides the graph.** It was taken for a stopped merge, and the
  empty merge tool replaced the list the commits are judged from.

### Stash part of your changes

Right-click a file in the working copy and choose **Stash this file** to put that one away and
keep working on the rest; a new file goes too. **Stash the staged changes** (in the same menu
and the palette) stashes exactly what is staged and leaves everything unstaged where it is,
even in the same file. A file whose name has `*` or `?` in it is that file, never a pattern.

### See an image change

A changed image now shows before and after side by side, with each side's size in pixels and
bytes, instead of "Binary file — no textual diff". PNG, JPEG, GIF, WebP, BMP, ICO and AVIF are
drawn, for a commit, a comparison, and staged or unstaged changes alike. An added or deleted
image says which side it is missing from, and transparency shows against a checkerboard.

### See whether a pull request passes its checks

Each open pull or merge request in the sidebar carries a mark for its latest commit's checks: a
tick when they passed, a cross when one failed, a clock while they run. Hover it for the count
("1 of 3 checks failed"); click it to open the failing check. GitHub's check runs and commit
statuses are read together, and GitLab's newest pipeline for the commit. The proposals list
appears first, and the marks follow as the host answers.

### Coral can update itself

About has an **Install and restart** button once a newer release is found, and the palette has
**Check for updates** and, after a check finds one, **Install Coral … and restart**. The
download is verified against Coral's signing key before anything is replaced. An AppImage
replaces itself; a .deb asks for your password and installs with dpkg; the macOS app and the
Windows installer update in place. A build from source is offered the release page.

Coral still asks GitHub nothing until you press the button. **Check for updates when Coral
starts**, in About, does it at launch, and a newer release then shows in the status bar.

## 1.2.3

### The window looked older than it is

The interface has been redrawn without moving anything: every pane, control, shortcut and
gesture is where it was.

- **Cards.** The commit list, a diff or the merge tool, the right panel, the terminal and a
  Preferences pane are rounded cards on the window's ground, instead of panes meeting at
  full-height rules. The sidebar sits on the ground, and the splitters are the gaps between
  cards.
- **The right panel is white**, like the list beside it. Its sections, and the sidebar's, are
  headed by small uppercase labels, and Path/Tree is a segmented switch.
- **Branch pills** are tinted in their lane's colour with a coloured edge, rather than carrying
  a solid block of it. The checked-out branch is still filled.
- **A subject's prefix**, such as `ui:` or `smb/client:`, is set back in dim mono so the sentence
  after it leads.
- **Floating panels** (menus, the palette, prompts and toasts) are rounder, with rows of one
  height.
- **Preferences** panes share one padding. Appearance had none, so its heading touched the list.
- **Merge tool:** the conflict being resolved was outlined line by line. It now carries a wider
  bar down the gutter.

The lanes keep the elbows and the initialled disc on every commit. Their initials are now set in
the interface's own face.

### Double-clicking the title bar did nothing

It maximises and restores the window again, as a desktop's own title bar does. It had been
turned off because the strip also carries the tabs, and a tab is somewhere people click twice
by accident; a double click on a tab or a button is still left alone.

### Security

- **rustls is 0.23.46**, for RUSTSEC-2026-0346, -0347, -0348 and -0349: session secrets in
  `Debug` output, data sent after an ECH rejection, a Brotli allocation before authentication,
  and a downgrade to classical key exchange.
- **rustls-webpki is 0.103.16** and **webpki-root-certs 1.0.10**, for RUSTSEC-2026-0382 and
  -0383: distrusted intermediates could still be accepted.

## 1.2.2

### A new repository under a profile was left on the ssh agent

A profile carries an identity, signing settings and an ssh key, and a repository created under
one was given the first two. So a repository started under a work profile that pins a key went
out on whatever the agent offered first — the failure pinning a key exists to prevent, arriving
at a repository nobody had had the chance to configure yet. It carries all three now.

A clone was never wrong about which key it used, but stamping it with the profile's settings
put the profile's *public* half into a repository the form had chosen a different key for. git
never reads that half; it is the one the settings pane offers for pasting into a host, so the
pane named a key the repository does not sign in with. The key a clone used now decides both
halves.

### Security

- **rustls is 0.23.45**, for RUSTSEC-2026-0285: TLS 1.3 handshake messages were accepted across
  encryption level boundaries. It reaches every build twice, through the client that talks to
  GitHub and GitLab and through tauri.

## 1.2.1

### A submodule was cloned with whatever key the agent offered

A repository cloned over ssh with a chosen key records that key in its own config, and a
submodule reads none of it. git clones and fetches each submodule in a child process running in
that submodule's configuration, which is a different repository; the superproject's is not part
of it, and a `-c` does not survive the way in either. So the first "Fetch a working copy" on a
private submodule authenticated perfectly well as whoever the agent happened to offer first,
and the host answered that the repository does not exist — which reads as a wrong URL or a
missing grant, never as the wrong key.

The key is handed to the update in the environment now, which is what git does carry into a
submodule, and is written into each submodule's own clone so that a fetch from inside one finds
it too. It is cleared there as well as written: a submodule left holding a key the repository
has moved off is the same silent failure again.

A submodule that really does live on another host can be given a key of its own, from the dots
beside it or from its panel, which now says which key reaches it and what a pinned key costs.

## 1.2.0

Ninety-four fixes, most of them found by driving the real window rather than by running the
tests. The ones worth reading about first are the handful that were quietly losing work.

### Resolving a conflict could destroy what it touched

- **Taking a side of a conflicted symlink wrote into the file it pointed at.** What the index
  holds for a link is the path it points at, and those bytes were written into the worktree
  with an ordinary write while the old link was still there: the new target landed inside
  whatever the old link pointed at, and that file's contents were gone. The link itself never
  moved, so the index was then resolved to the side nobody asked for, and Coral said the file
  was settled.
- **A merge that conflicted in a submodule could not be finished at all.** Its index stages are
  commits, and reading them as content came back empty, so the pane put "not conflicted" in red
  over a view that never stopped loading and offered deletion as the only way out — which would
  have taken the submodule out of the superproject.
- **Git LFS, three ways.** A conflicted asset was laid out as the three lines of its pointer
  with a checkbox on each, so taking the object from one side and the size from the other made
  a pointer naming nothing: it commits, it pushes, and the next clone has no file there. The
  same pointer could be part-staged from the diff view. And resolving any conflict left the
  pointer on disk where the asset should be.
- **A resolved file came out in the repository's form rather than the worktree's.** In a
  `core.autocrlf` checkout, which is what git for Windows gives every clone by default, the
  file came out with Unix line endings among Windows ones. Nothing reported it, because
  cleaning those bytes again gives back exactly what the index holds.
- A side is taken through the index now and the file laid down from it, so the mode survives
  and nothing is written through anything. That also settles a conflict on an executable, whose
  bit used to depend on what the merge happened to leave on disk.

### An apostrophe in a path broke three things

git runs a `!`-prefixed credential helper, `GIT_SEQUENCE_EDITOR` and `core.sshCommand` through
a shell, so what Coral puts in them is shell syntax. Paths went in between single quotes with
nothing done about the apostrophe that might be in one.

- **Nobody called O'Brien could sign in.** Every fetch, push and clone needing a credential
  failed, and the message named the remote rather than the helper.
- **A repository under `Bob's game` could not have its history edited.** Every drop, reword and
  reorder died on "There was a problem with the editor".
- **A key under such a home directory could not be used**, because pinning it produced an ssh
  command the shell could not parse.

### Signing

- **A repository that signs its tags could not be given a plain one.** `tag.gpgSign` turns
  every tag into a signed one needing a message git opens an editor for, and the window pins
  the editor to a no-op so nothing can hang, so "Create tag here" answered "fatal: no tag
  message?" and made nothing.
- **An empty signing override was written rather than cleared.** git runs what
  `gpg.ssh.program` names without looking at it first, so an empty value is a program whose
  name is the empty string, and every signed commit then fails with `cannot run :`.

### Large repositories

- **Opening a merge read every conflicted file whole to look at 8000 bytes of it.** One
  conflicted 120 MB asset made the window hold 358 MB to decide the file had no lines worth
  showing. Sizes are asked for first now, and anything past the size a patch is shown at is
  taken whole without being read: 7 MB and under ten milliseconds for the same file.
- **Opening a large new file read the whole of it to say it was too large.** An untracked file
  is diffed against nothing, so its patch is as long as the file, and it was held in full
  before the guard looked at it. A 300 MB log dropped into a repository took the window from
  192 MB to 485 MB on one click.
- **Only the first edit of a file reached the window.** File events are narrowed against a
  fingerprint taken from `git status`, which names which paths differ and never what they now
  hold, so a file already listed as modified hashed the same however many times it was saved.
  A diff left open showed the first edit for as long as it stayed open, and staging a hunk from
  it built a patch against content that had moved on.

### Everything else

Sixty more, nearly all of them things the window said that were not so: a conflict in a CRLF
file that hid the incoming side entirely, a token standing alone in a remote URL reaching the
logs, Enter answering the two questions nothing can undo, rewording a commit throwing its body
away, a dropped stash staying in the list, undo naming one operation and undoing another, and a
long run of labels cut off mid-word. The command line gained `--mainline`, without which a
merge could not be reverted from it, and the hunk and line selection for discarding that the
window already had.

## 1.1.2

Two faults found by driving the window, and the last of the workflows that had never run.

- **The change map sat on top of blame, pointing at the wrong rows.** The strip down the right
  of a side-by-side diff maps where the changes are and jumps to them. Its marks, its position
  band and its jump target are all measured against the diff's rows, and blame puts the whole
  file in the same scroller instead, so on anything long enough for the diff to be windowed the
  strip said the reader was somewhere they were not. Whether it appeared at all followed the
  inline-or-side-by-side toggle, which blame does not show, so it turned up according to a
  setting that was not on screen.
- **A commit button that could not work would not say why.** With nothing staged it says "Stage
  something to commit". With files staged and the summary box empty it stayed disabled and read
  "Commit 2 files", naming what it would have done rather than what was missing.
- **The nightly kernel scenarios had never once started.** That workflow never built the command
  line binary, and the scenario script looks for it and exits before the first scenario when it
  is not there.
- A watcher test assumed the first notification after a commit carries the ref and index move.
  The watcher may deliver more than one, and on Windows the file write arrives on its own often
  enough that the same commit passed on one branch and failed on a tag.

## 1.1.1

Forty-six fixes and the pipeline that builds them. Coral now ships for five platforms rather
than for whichever one the release was cut on, and almost every fault below came from driving
the window or from watching the build run somewhere nobody develops.

### It builds for five platforms now

- **Linux on x86-64 and ARM64, macOS as one universal bundle, Windows on x86-64 and ARM64.**
  Every one of them on hardware GitHub gives away, so the set costs nothing to keep.
- **The Linux bundles ask less of the machine they land on.** They are built on the oldest
  distribution still supported rather than the newest available, which takes the glibc floor
  from 2.39 to 2.34: the difference between refusing to start on Debian 12, Ubuntu 22.04 and
  RHEL 9, and running on all three.
- **A release could not have been published at all.** The workflow built bundles and then
  uploaded them somewhere that expires and gives nobody a link, because it never named a tag.
  Publishing is one job now, holding the only permission that can write, and a tag that does
  not name the version the binaries carry is refused before anything is built.
- **The command line ships everywhere.** It reached `/usr/bin/coral` through the Debian
  package alone, so every other platform had no way to get it.

### What building elsewhere found

- **The test suite did not compile on Windows**, in three places, so that half of the promise
  had never been checked at all.
- **Cancelling a clone leaves its half-cloned repository behind on Windows.** Not fixed:
  killing a process group is implemented for unix only, so git's own children outlive the
  cancel and hold the directory open. It is written down where the next person will find it.
- **The licence check had never once run.** Its tool installs no binary unless a feature is
  named, says so in a warning, and exits successfully, so the failure only appeared much later
  as a missing command. The same line in the README told everyone else to install it that way.

### The window

- **Clicking a tag on the kernel did nothing.** The ref was found, the scroll went to it, and
  the selection was thrown away by the frame that arrived next. Selecting by object id rather
  than by row number survives the paging.
- **A reword lost its message when that commit also conflicted.** git finishes the rebase in
  one continue after a conflicted edit, so a message amended afterwards was amended onto
  nothing. It is written before the continue now.
- **A tab that failed to open left the previous repository on screen**, under the new tab's
  name, which is the worst way to be wrong about which repository you are looking at.
- **Clicking the title strip maximised the window.** The pointer went to the window manager on
  the press, so a click was a drag of no distance, and a drag at the top of the screen snaps.
- **A merge left its message in the commit box after it was committed**, so the next commit
  started with the last one's words.
- **Twelve things the window said that were not true**, from a status line describing a
  repository the tab was not showing, to a crumb that cut all three words of a submodule's
  name, to failures that were all called "Something went wrong".
- **Six controls that could not work were offered anyway**, including pushing a branch that
  was not checked out and two buttons with nothing behind them.
- **A repository can go while its tab is open**, which used to be a window that never
  recovered.

## 1.1.0

The window is redrawn from the tokens up. Colour, type, spacing, shape and motion come from
one measured set rather than from literals in thirty-two components; every control is a drawn
shape rather than a typeface glyph; and the theme follows the desktop. As with the releases
before it, nearly all of it came from driving the window rather than from reading the code —
including six faults that only the real WebKitGTK window would show.

### The look of it

- **A design system with numbers behind it.** Aqua does every functional job — what is
  selected, what has focus, what a primary button is — and coral carries the identity: the
  mark, the active tab's edge, the ring on the commit you are on, and the working copy's
  marker. They are kept apart by role rather than by hue, because a warm red cannot mean
  "brand" in one place and "this deletes something" in another. Every pair a reader has to
  tell apart is asserted in `contrast.test.ts` rather than judged by eye.
- **An icon set drawn in the graph's own vocabulary.** The controls were typeface glyphs —
  `↶`, `⤓`, a literal `P` for patch, four emoji in the sidebar — which came out differently on
  every machine and had no bold cut to ask for. Sixty-two shapes on one grid now, in the
  vocabulary the canvas already draws a million times over: stroke the structure, fill the
  subject. Weight is derived from the size asked for, so a glyph on an eleven-pixel pill and
  one in a twenty-pixel button land on the same optical weight.
- **A mark that is a commit graph and a piece of coral.** The old one was a Y, which is the
  branch glyph every client already uses, this one included. Four nodes now, with the trunk
  carrying on past the two that leave it.
- **The type ships with the application.** Inter and JetBrains Mono are bundled rather than
  asked of the machine, and the monospace face has its ligatures off: drawing `!==` as one
  glyph is taste in an editor and a lie in a diff.
- **Menus, dialogs and the palette lift off the page.** Fourteen places wrote their own wash
  of black or their own shadow, at four different strengths, all of them plain black over a
  page that is nearly black already.

### The graph and the lists

- **The commit you are on is ringed**, in the brand colour, clear of the node's own. The
  specification has said so since its first draft and it was never drawn.
- **The commit list loses its column headers.** Three words in uppercase over a list whose
  columns are a pill, a drawing and a sentence, costing twenty-six pixels of every screen.
  The handles that size the columns move outside the scroller and run its full height.
- **The sidebar reads as a list of branches, not a table.** Sentence case at the body size,
  drawn icons in place of the emoji, and one left edge for every section: the tick that marks
  the branch you are on used to be added only to that row, so the one name anybody is looking
  for started fourteen pixels further right than the rest.
- **The lane ribbons stop shouting.** The tint that labels a run of commits with its branch
  was a seventh of the lane's stroke strength, which is right for a thirty-pixel pill and
  wrong for a two-hundred-pixel ribbon beside a message.
- **Density is a choice** — compact, default or comfortable — and it reaches the lanes as well
  as the rows, which are laid out by two systems that cannot read each other.

### Reading a change

- **The words that changed inside a replaced line are marked.** A one-word edit and a
  rewritten line used to look identical. The comparison is over words rather than characters,
  so a renamed method reads as one changed name instead of confetti; a line where most of the
  words changed is left alone, because marking nine tenths of it only repeats what the line's
  colour said.
- **The commit panel leads with who wrote it**, on the same disc the canvas fills a node with,
  from the same eight colours by the same hash — so the row that was clicked and the panel
  that answers wear one mark between them.
- **The conflict tool, the staging panel and the diff header** are on the one set of shapes.
  The diff header carried six controls in four outlines and three corner radii across
  thirty-four pixels.

### Room on a small screen

- **The toolbar is one row of icons**, starting at the breadcrumb rather than floating on the
  window's centre line, in about half the width it took. The words move into the tooltip:
  the name, then what the action does, then the keystroke, read from the same table the help
  overlay is built from.
- **Both side panels fold away from the toolbar**, and the left one has a third state between
  showing and gone: a thirty-six pixel rail of its section icons, each opening the panel at
  its own section. The two panels take three hundred pixels between them, which on a laptop
  is most of a diff.

### Correctness

- **Coral opened light on a dark desktop** and stayed there. The stored value is a choice now
  rather than a theme — follow the desktop, always light, or always dark — and a `matchMedia`
  listener means a desktop that goes dark in the evening takes the window with it.
- **The lanes drifted off the rows they belong to** at any density but the default. The canvas
  fitted its metrics from the shipped row height rather than the caller's, so every node sat a
  little further from its own commit than the last.
- **The working copy row sat on top of the first commit**, held twenty-two pixels below the
  top of the list to clear a column header that no longer exists.
- **The working copy row and a row the search had matched** were nearly the same colour in the
  dark theme, where both tints are dark browns. They are told apart by shape now: the search
  tints a row, the working copy is ruled.
- **Text went soft on eleven surfaces.** WebKit antialiases with subpixel precision only where
  it knows what is behind the text, so a surface with no background of its own drops to
  grayscale. `style.test.ts` holds the rule that every one of them paints its own.
- **Hiding the left panel was permanent in the dev gallery**, because the fixture engine
  answered `null` for a command it had never heard of and the sidebar threw on it. It throws
  instead, which is why five missing fixtures were found at all.

### Underneath

- **Every surface of the window is photographed in a real browser.** `just window-test` drives
  Chrome over the real application on the fixture engine and takes twenty-six pictures in both
  themes, with no repository, no Rust and no network. It adds nothing to `package.json`: Node
  has a global `WebSocket`, so speaking the DevTools protocol needs no package.
- **The tests are type-checked.** A suite of eight hundred was the only TypeScript in the
  repository that nothing checked.
- **`App.svelte` is four hundred lines smaller**, across six modules that are each about one
  thing: what to call a ref, which labels a row shows, what the lane column costs the row it
  sits in, what a revision offers, what discarding promises, and the window's own top line.

## 1.0.2

Two lives on one machine, a repository taken without all of it, and a menu that offers what
can actually happen. As with 1.0.0, nearly all of it came from driving the window rather than
from reading the code.

### The window

- **Profiles.** Work and personal on one machine: each keeps its own tabs, groups and recent
  repositories, and its own identity for repositories cloned under it. Switching points the
  workspace at another directory rather than teaching the tabs about profiles.
- **A work account and a personal one on the same host.** Hosting tokens were filed under the
  host's origin alone, so github.com held exactly one. They now carry the profile, and a
  profile that has never signed in falls back to the one the command line writes. The status
  bar's host chip opens the sign-in, which the window had no way to reach at all.
- **Watch a fetch, a push or a clone, and stop it.** Progress reaches the window, cancelling
  reaches the git it started, and a cancelled operation is recorded as cancelled rather than
  as a failure.
- **Choose the shell the terminal opens**, and whether it reads the login files. A shell
  already running keeps what it started with, so the pane has a button that starts it again.
- **Take a repository without all of it.** The clone form and `coral clone` offer recent
  history only, which is a depth, and history now with file contents on demand, which is a
  blobless filter. They are different economies and are one choice rather than two switches.

### Correctness

- **Undo reaches a commit**, which is the commonest thing anyone wants back. It behaves as a
  soft reset: the branch moves, the work returns to the index.
- **A pinned ssh key lost to one named in the user's own config.** `-i` and a `Host` block's
  `IdentityFile` accumulate and the agent reorders them, so the wrong account authenticated.
  Only `-F none` makes the choice authoritative.
- **A stash that will not apply has stopped, not failed.** git exits 1 for a conflict and for
  a real failure alike; the repository, not the exit code, tells them apart. The merge tool
  opens on it, and hides Continue and Abort where git has no operation to continue.
- **A moved tag could not be published.** `--force-with-lease` is compared against the
  remote-tracking ref and git keeps none for a tag, so every forced tag push was refused. The
  lease now names what the remote holds.
- **A ref the branch has passed offered every way back to it.** Right-clicking a release tag
  on an up-to-date branch offered a fast-forward git refuses. The menu asks where the ref
  stands and turns the fast-forward around; a branch is taken along without being checked out,
  a tag is replaced, and where neither can happen the line says which way round they are.
- **The graph went stale after a pull**, and blank after a tag was fast-forwarded from far
  down the history. Both were the reload deciding whether to keep the rows on screen.

### Saying what happened

- A rejected push said it stopped on conflicts, in a status line that phrased outcomes itself
  rather than asking the module that owns the wording. Both now say the same thing.
- The rejection dialog explained a tag as though it were a branch, and offered to pull, which
  does nothing about a name the remote already has.
- A modified run in the diff overview strip was drawn in a graph lane colour. It is now the
  two colours the diff itself uses.
- `coral clone` and `coral init`, so the command line can do everything the window can.

## 1.0.0

Coral draws its own title bar, renames a branch, deletes a tag on its remote, and takes back a
reset. Most of what follows it, though, came from driving the window as a user rather than as
a test: scenarios on real repositories, on the kernel, on shapes nobody sets out to build, and
on two developers sharing one remote.

### The window

- **One row where there were three.** The desktop's title bar, Coral's own header and the tab
  strip each took a row to say the window was called Coral. The tabs are the title bar now, at
  forty pixels, carrying the mark, the tabs, the log, preferences and theme buttons, and
  minimise, maximise and close. Dragging is handled in the page so a stray double click does
  not throw the window to full screen, and right-clicking the strip hands the bar back to the
  desktop for a window manager that handles an undecorated window badly.
- **Rename a branch.** The engine and the CLI have always been able to; the window could not.
- **Delete a tag on its remote**, which was the missing half of pushing one.
- **Undo a reset.** A soft or mixed reset leaves the worktree dirty by construction, and undo
  refused on a dirty worktree, so the action people most want back was the one they could never
  have. The guard now asks whether the restore would overwrite anything rather than whether
  anything is dirty — including whatever is staged, which is a place work can exist in only one
  copy.
- **A repository with nothing in it says so**, instead of an empty pane beside a panel offering
  to show the details of a commit that could be selected.
- The start page is a column with a measure in the middle of the window, rather than a strip in
  one corner of a large empty rectangle.

### Correctness

- **A generated patch quotes a path the way git quotes it.** Staging or discarding a hunk in a
  file whose name holds a newline or a tab built a header git could not read, so it staged
  nothing — or, worse, silently lost half the name.
- **A conflicted path is read rather than refused.** Opening a conflicted file showed
  "malformed git diff output" where the conflict should have been.
- **Finishing a conflicted merge or rebase is journalled**, so undo can take it back instead of
  reaching past it to an older entry and failing in git's words under a button marked Undo.
- **A write waits out an index lock another git holds for a moment.** The file watcher runs a
  status on every change, and a status beside a commit made the commit fail.
- **A password in a remote URL never reaches a message.** The argv was always redacted; git's
  own output was not.
- The last row of the graph can be scrolled to — on the kernel that is its first commit — and
  the commit message column can no longer be squeezed out of existence in a narrow window.

### Saying what happened

- An operation that stopped is no longer logged as finished, so a rejected force push does not
  read like one that went through.
- A failure explains itself when git explains itself on stdout rather than stderr, which is
  what a conflicting stash pop does.
- Object ids in labels are short everywhere, conflicted files are listed once rather than twice,
  a waiting toast no longer sits on the commit button, and the fast-forward menu hint no longer
  reads as a refusal of the thing it is about to do.

## 0.2.0

The release that made the graph somebody else's repository rather than only a well-behaved one,
and gave the window the operations that were still only in the engine.

### The graph

- **Solo and hide.** A branch or tag can be walked on its own, or taken out of the walk
  entirely; either choice is remembered per repository. The panel says which, with a banner
  naming the soloed branch and an eye on every row that stays showing once a ref is hidden.
- **Shallow clones open.** A clone with no history past its boundary used to fail outright,
  which is every module in a `west` workspace. The boundary is grafted the way git grafts it.
- **Drag and drop between pills.** A branch onto its tracking branch pushes, the other way
  round pulls, and anything else asks whether to merge, rebase, or open a request.
- **Delete a branch from a remote** from its own row, behind a dialog that says the deletion
  is for everyone and that nothing local is touched.
- **A rejected push offers a way out.** The window had no force at all, so a non-fast-forward
  rejection could only be resolved in a terminal. It now shows git's reason and offers to pull
  and rebase or to force, and the push menu carries a deliberate force as well. Always
  `--force-with-lease`.
- **Search by path.** The filter now also finds commits by the files they touched, alongside
  the message, the author and the object id, and the four passes run together.
- **Tags newest first**, by version rather than by name, so a repository with a thousand of
  them opens on the ones somebody wants.
- The lane colour runs from a commit's node to its message rather than tinting the message
  itself, so the selected row is findable again on a coloured graph.

### Working with changes

- **Patches.** Two commits picked write out the series between them; the same button takes a
  series back in, either as commits keeping their authors or as changes left in the working
  copy. A patch that no longer applies stops in the conflict tool.
- **Amending starts from the message it replaces** instead of an empty box.
- **A unified diff widens to the whole file**, and steps change by change in both layouts.
- **An embedded terminal**, opening in the repository, on Ctrl+`.

### Hosting

- **Open a pull or merge request** from a branch, on a form that names where it would land,
  with a title, a description and a draft switch.

### The window

- **Tabs look like a browser's**: rounded above, flared at the base, the current one merging
  into the toolbar. Each carries a picture — a branch unless another of twelve is picked from
  its menu — coloured from the repository's path so a strip of tabs is scannable.
- **Every keyboard shortcut the panel lists now does something.** It listed several that did
  not.
- **About** says which build this is: version, commit and whether it is a debug build.
- Commit signing configured per repository and inherited from the app.

### Fixed

- Clicking a commit's message did nothing; only the author's initials selected the row.
- A collapsed tab group's count was invisible, painted in the strip's grey on the strip's grey.
- The breadcrumb cut the tails off `g`, `p` and `y` and the foot off a slash.
- One repository's commit could appear under another repository's name.
- Transfer progress overwrote rebase progress.
- A long line pushed the other pane off the side in side-by-side.

## 0.1.0

The first version: the graph, the operations, conflicts, remotes, hosting, tabs and the CLI.

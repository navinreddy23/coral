# Measuring Git clients

`measure.py` opens a repository in a Git client and measures three things: how long until the
screen stops changing, how much memory the client holds once it has, and how much CPU it
uses doing nothing afterwards. The website's comparison comes from `results.json`, which
records every run on the machine named in it.

The method is the same for every client:
- Each client runs alone on a nested X server, so the screen being watched holds that client
  and nothing else.
- Each run starts from a copy of a throwaway home in which the client's first-run setup was
  already done. None of the user's own settings, keys or accounts are reachable.
- **Time to still** runs from launch to the screen's last change before five seconds without
  one. That is the moment the repository is on screen and the client has stopped painting.
  It is not when every background task has finished: Coral, for one, keeps walking the
  history after the first screen is painted.
- **Memory** is the proportional set size of every process carrying the run's home in its
  environment, taken twenty seconds after the screen stops changing. Proportional means each
  shared library is counted once, split between the processes that share it.
- **Idle CPU** is the CPU time those processes use over the next thirty seconds with no input,
  as a percentage of one core.

To re-run:

```sh
Xephyr :9 -screen 1500x950 -ac -noreset &
python3 -m venv /tmp/venv && /tmp/venv/bin/pip install python-xlib
git clone ~/.cache/coral-bench/linux /tmp/cb/kernel     # `just kernel-clone` makes the source
git -C /tmp/cb/kernel commit-graph write --reachable
/tmp/venv/bin/python tests/clients/measure.py coral . /tmp/cb/kernel /tmp/cb/out/coral
```

For any other client, pass the directory it is unpacked in, and `--template` a home in which
its first-run setup is done.

Keep the work directory short. GitKraken puts a Unix socket under it, and a socket path longer
than 108 bytes fails with `EINVAL`.

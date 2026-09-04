import { experimentalGit, experimentalSetGit, type GitChoice, type GitView } from '../ipc/experimental';
import { messageOf } from '../ipc/error';

/** Which git Coral runs, and what else is available to run. */
export class ExperimentalState {
  view = $state<GitView | null>(null);
  busy = $state(false);
  error = $state<string | null>(null);

  async load(): Promise<void> {
    await this.#run(experimentalGit);
  }

  async chooseGit(choice: GitChoice): Promise<void> {
    await this.#run(() => experimentalSetGit(choice));
  }

  async #run(action: () => Promise<GitView>): Promise<void> {
    this.busy = true;
    this.error = null;
    try {
      this.view = await action();
    } catch (e) {
      this.error = messageOf(e);
    } finally {
      this.busy = false;
    }
  }
}

import { hostingReply, hostingReview, type Remark, type Reply } from '../ipc/commands';
import { messageOf } from '../ipc/error';

/**
 * One proposal's review: what has been said, and saying something back.
 *
 * Read again after every reply rather than having the reply appended here: the host decides
 * what a reply became — an approval with text is two entries on GitLab and one on GitHub — and
 * the list should show what is there, not what was meant.
 */
export class ReviewState {
  remarks = $state<Remark[]>([]);
  loading = $state(false);
  busy = $state(false);
  error = $state<string | null>(null);

  #repo = '';
  #number = 0;
  #token = 0;

  async load(repo: string, number: number): Promise<void> {
    this.#repo = repo;
    this.#number = number;
    const token = ++this.#token;
    this.loading = true;
    this.error = null;
    try {
      const remarks = await hostingReview(repo, number);
      if (token === this.#token) this.remarks = remarks;
    } catch (e) {
      if (token === this.#token) this.error = messageOf(e);
    } finally {
      if (token === this.#token) this.loading = false;
    }
  }

  /** Sends a reply and reads the review again; true when the host took it. */
  async reply(reply: Reply): Promise<boolean> {
    this.busy = true;
    this.error = null;
    try {
      await hostingReply(this.#repo, this.#number, reply);
    } catch (e) {
      this.error = messageOf(e);
      return false;
    } finally {
      this.busy = false;
    }
    await this.load(this.#repo, this.#number);
    return true;
  }
}

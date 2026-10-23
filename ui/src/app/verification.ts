import type { Verification } from '../ipc/types';

export type Tone = 'ok' | 'warn' | 'danger';

/** What the details panel says about a commit's signature, or null when it has none. */
export function describe(v: Verification): { text: string; tone: Tone; why: string } | null {
  switch (v.kind) {
    case 'unsigned':
      return null;
    case 'good':
      if (v.trusted) return { text: `Good signature from ${v.signer}`, tone: 'ok', why: '' };
      return {
        text: v.signer === '' ? 'Good signature, from an unknown key' : `Good signature from ${v.signer}`,
        tone: 'warn',
        why:
          v.signer === ''
            ? 'The key is not among the allowed signers git was given.'
            : 'The key is not one git has been told to trust.',
      };
    case 'bad':
      return {
        text: 'Bad signature',
        tone: 'danger',
        why: 'The commit does not match the signature it carries.',
      };
    case 'expired':
      return { text: `Signed by ${v.signer}, with an expired key`, tone: 'warn', why: '' };
    case 'revoked':
      return { text: `Signed by ${v.signer}, with a revoked key`, tone: 'danger', why: '' };
    case 'unchecked':
      return { text: 'Signed, but git could not check it here', tone: 'warn', why: v.reason };
  }
}

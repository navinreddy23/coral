import { describe as suite, expect, it } from 'vitest';

import { describe } from '../src/app/verification';

suite('what the panel says about a signature', () => {
  it('says nothing for an unsigned commit', () => {
    expect(describe({ kind: 'unsigned' })).toBeNull();
  });

  it('names a trusted signer plainly', () => {
    expect(describe({ kind: 'good', signer: 'Ada <ada@example.test>', trusted: true })).toEqual({
      text: 'Good signature from Ada <ada@example.test>',
      tone: 'ok',
      why: '',
    });
  });

  it('marks a valid signature from a key nobody vouched for', () => {
    const said = describe({ kind: 'good', signer: '', trusted: false });
    expect(said?.tone).toBe('warn');
    expect(said?.text).toBe('Good signature, from an unknown key');
  });

  it('passes on git\'s reason when it could not check', () => {
    const reason = 'gpg.ssh.allowedSignersFile needs to be configured';
    expect(describe({ kind: 'unchecked', reason })?.why).toBe(reason);
  });

  it('calls a bad signature bad', () => {
    expect(describe({ kind: 'bad', signer: '' })?.tone).toBe('danger');
  });
});

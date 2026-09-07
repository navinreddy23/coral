// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));

import Transfer from '../../src/app/Transfer.svelte';
import { TransferState } from '../../src/state/transfer.svelte';

function bar(over: Record<string, unknown> = {}) {
  const transfer = new TransferState();
  transfer.take({
    key: '/srv/thing',
    label: 'Clone',
    state: 'running',
    phase: 'Receiving objects',
    current: 40,
    total: 200,
    percent: 20,
    remote: false,
    detail: '',
    ...over,
  });
  return { transfer, ...render(Transfer, { props: { transfer } }) };
}

describe('the transfer bar', () => {
  it('is not there when nothing is running', () => {
    const { container } = render(Transfer, { props: { transfer: new TransferState() } });
    expect(container.querySelector('.transfer')).toBeNull();
  });

  it('draws how far along it is', () => {
    const { container } = bar();
    const fill = container.querySelector('.fill') as HTMLElement;
    expect(fill.style.width).toBe('20%');
    expect(container.textContent).toContain('40 / 200');
  });

  it('offers the way out before there is anything to draw', async () => {
    // The case it exists for. A host that never answers reports no progress at all, so a stop
    // that only appeared with the first record would never appear.
    const { container } = bar({ phase: '', current: 0, total: 0, percent: 0 });
    expect(container.querySelector('.fill'), 'nothing to claim yet').toBeNull();
    expect(container.querySelector('.track.waiting')).not.toBeNull();

    const stop = container.querySelector('.stop') as HTMLButtonElement;
    expect(stop.disabled).toBe(false);
    await fireEvent.click(stop);
    expect(invoke).toHaveBeenCalledWith('cancel_transfer', { key: '/srv/thing' });
  });

  it('cannot be asked to stop twice', async () => {
    invoke.mockReset();
    invoke.mockResolvedValue(true);
    const { container } = bar();
    const stop = container.querySelector('.stop') as HTMLButtonElement;

    await fireEvent.click(stop);
    expect((container.querySelector('.stop') as HTMLButtonElement).disabled).toBe(true);
    expect(container.textContent).toContain('Stopping');
  });
});

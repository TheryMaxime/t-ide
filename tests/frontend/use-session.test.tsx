/**
 * Tests for the desktop WebSocket session hook (T037).
 */

import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { reduce, useSession, type SessionState } from '../../src/hooks/useSession';
import type { ServerMessage } from '../../src/types/protocol';

/** Minimal in-memory stand-in for the browser WebSocket. */
class FakeSocket {
  sent: string[] = [];
  closed = false;
  onmessage: ((event: MessageEvent<string>) => void) | null = null;
  onclose: (() => void) | null = null;

  send(payload: string) {
    this.sent.push(payload);
  }

  close() {
    this.closed = true;
  }

  emit(message: ServerMessage) {
    this.onmessage?.({ data: JSON.stringify(message) } as MessageEvent<string>);
  }
}

const initial: SessionState = {
  connection: 'reconnecting',
  sessionId: null,
  running: false,
  entries: [],
  approval: null,
  error: null,
};

function entry(order_index: number): ServerMessage {
  return {
    type: 'transcript.entry',
    session_id: 3,
    entry: {
      order_index,
      entry_type: 'response',
      origin_device_name: null,
      content: { text: `step ${order_index}` },
      created_at: '2026-09-01T10:00:00Z',
    },
  };
}

describe('reduce', () => {
  it('keeps transcript entries ordered and ignores duplicates', () => {
    const withSecond = reduce(initial, entry(1));
    const withFirst = reduce(withSecond, entry(0));
    const withDuplicate = reduce(withFirst, entry(0));

    expect(withDuplicate.entries.map((item) => item.order_index)).toEqual([0, 1]);
    expect(withDuplicate.sessionId).toBe(3);
  });

  it('tracks the running flag and pending approval', () => {
    let state = reduce(initial, { type: 'session.created', session_id: 3, status: 'running' });
    expect(state.running).toBe(true);

    state = reduce(state, {
      type: 'approval.request',
      session_id: 3,
      request: {
        id: 5,
        request_type: 'file_change',
        details: { path: 'a.txt', operation: 'write' },
        expires_at: '2026-09-01T10:05:00Z',
      },
    });
    expect(state.approval?.id).toBe(5);

    state = reduce(state, {
      type: 'approval.responded',
      request_id: 5,
      resolved_by_device_name: 'desktop',
      decision: 'approved',
    });
    expect(state.approval).toBeNull();

    state = reduce(state, { type: 'transcript.complete', session_id: 3 });
    expect(state.running).toBe(false);
  });

  it('surfaces protocol errors', () => {
    const state = reduce(initial, {
      type: 'error',
      code: 'SESSION_BUSY',
      message: 'a session is already running for project demo',
      busy_project: 'demo',
    });
    expect(state.error).toContain('demo');
    expect(state.running).toBe(false);
  });
});

describe('useSession', () => {
  it('sends the contract frames for create, prompt, cancel, and approvals', () => {
    const socket = new FakeSocket();
    const factory = () => socket as unknown as WebSocket;
    const { result } = renderHook(() => useSession('ws://test/ws', factory));

    act(() => result.current.createSession(1, 2, 'add a greeting'));
    expect(JSON.parse(socket.sent[0])).toEqual({
      type: 'session.create',
      project_id: 1,
      provider_id: 2,
      prompt: 'add a greeting',
    });

    act(() => socket.emit({ type: 'session.created', session_id: 7, status: 'running' }));
    expect(result.current.sessionId).toBe(7);
    expect(result.current.running).toBe(true);

    act(() => result.current.sendPrompt('and run the tests'));
    expect(JSON.parse(socket.sent[1])).toEqual({
      type: 'prompt.send',
      session_id: 7,
      text: 'and run the tests',
    });

    act(() => result.current.respond(4, 'denied'));
    expect(JSON.parse(socket.sent[2])).toEqual({
      type: 'approval.respond',
      request_id: 4,
      decision: 'denied',
    });

    act(() => result.current.cancel());
    expect(JSON.parse(socket.sent[3])).toEqual({ type: 'session.cancel', session_id: 7 });
  });

  it('streams transcript entries into state and reports a lost backend', () => {
    const socket = new FakeSocket();
    const factory = () => socket as unknown as WebSocket;
    const { result } = renderHook(() => useSession('ws://test/ws', factory));

    act(() => socket.emit({ type: 'connection.state', state: 'connected' }));
    expect(result.current.connection).toBe('connected');

    act(() => socket.emit(entry(0)));
    expect(result.current.entries).toHaveLength(1);

    act(() => socket.onclose?.());
    expect(result.current.connection).toBe('computer_unavailable');
  });
});

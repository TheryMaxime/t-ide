/**
 * Desktop WebSocket client hook (T037).
 *
 * Connects the desktop frontend to its own backend, mirrors the streamed
 * session state into React state, and exposes the User Story 1 commands
 * (create session, send prompt, cancel, answer approvals).
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import type {
  ApprovalRequest,
  ClientMessage,
  ConnectionState,
  Decision,
  ServerMessage,
  TranscriptEntry,
} from '../types/protocol';

/** Backend endpoint; the desktop backend listens on the loopback interface. */
export const DEFAULT_ENDPOINT = 'ws://127.0.0.1:8787/ws';

export interface SessionState {
  connection: ConnectionState;
  sessionId: number | null;
  running: boolean;
  entries: TranscriptEntry[];
  approval: ApprovalRequest | null;
  error: string | null;
}

export interface SessionApi extends SessionState {
  createSession: (projectId: number, providerId: number, prompt: string) => void;
  sendPrompt: (text: string, model?: string) => void;
  cancel: () => void;
  respond: (requestId: number, decision: Decision) => void;
}

/** Factory so tests can inject a fake socket. */
export type SocketFactory = (url: string) => WebSocket;

/**
 * Module-level default so the identity is stable: a factory created per render
 * would reconnect the socket on every state update.
 */
const browserSocket: SocketFactory = (url) => new WebSocket(url);

const initialState: SessionState = {
  connection: 'reconnecting',
  sessionId: null,
  running: false,
  entries: [],
  approval: null,
  error: null,
};

export function useSession(
  endpoint: string = DEFAULT_ENDPOINT,
  createSocket: SocketFactory = browserSocket,
): SessionApi {
  const [state, setState] = useState<SessionState>(initialState);
  const socketRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    const socket = createSocket(endpoint);
    socketRef.current = socket;

    socket.onmessage = (event: MessageEvent<string>) => {
      setState((current) => reduce(current, JSON.parse(event.data) as ServerMessage));
    };
    socket.onclose = () => {
      setState((current) => ({ ...current, connection: 'computer_unavailable', running: false }));
    };

    return () => {
      socketRef.current = null;
      socket.close();
    };
    // The socket is rebuilt only when the endpoint changes; `createSocket` is a
    // stable factory supplied once by the caller.
  }, [endpoint, createSocket]);

  const send = useCallback((message: ClientMessage) => {
    socketRef.current?.send(JSON.stringify(message));
  }, []);

  const createSession = useCallback(
    (projectId: number, providerId: number, prompt: string) => {
      setState((current) => ({ ...current, entries: [], error: null, approval: null }));
      send({ type: 'session.create', project_id: projectId, provider_id: providerId, prompt });
    },
    [send],
  );

  const sendPrompt = useCallback(
    (text: string, model?: string) => {
      setState((current) => {
        if (current.sessionId !== null) {
          send({ type: 'prompt.send', session_id: current.sessionId, text, model });
        }
        return current;
      });
    },
    [send],
  );

  const cancel = useCallback(() => {
    setState((current) => {
      if (current.sessionId !== null) {
        send({ type: 'session.cancel', session_id: current.sessionId });
      }
      return current;
    });
  }, [send]);

  const respond = useCallback(
    (requestId: number, decision: Decision) => {
      send({ type: 'approval.respond', request_id: requestId, decision });
    },
    [send],
  );

  return useMemo(
    () => ({ ...state, createSession, sendPrompt, cancel, respond }),
    [state, createSession, sendPrompt, cancel, respond],
  );
}

/** Apply one streamed frame to the session state. */
export function reduce(state: SessionState, message: ServerMessage): SessionState {
  switch (message.type) {
    case 'connection.state':
      return { ...state, connection: message.state };
    case 'session.created':
      return { ...state, sessionId: message.session_id, running: true, entries: [] };
    case 'prompt.sent':
      return { ...state, sessionId: message.session_id, running: true };
    case 'session.cancelled':
      return { ...state, running: false, approval: null };
    case 'transcript.entry':
      // Entries are keyed by order_index, so a replayed entry never duplicates.
      if (state.entries.some((entry) => entry.order_index === message.entry.order_index)) {
        return state;
      }
      return {
        ...state,
        sessionId: message.session_id,
        entries: [...state.entries, message.entry].sort(
          (left, right) => left.order_index - right.order_index,
        ),
      };
    case 'transcript.complete':
      return { ...state, running: false, approval: null };
    case 'approval.request':
      return { ...state, sessionId: message.session_id, approval: message.request };
    case 'approval.responded':
      return state.approval?.id === message.request_id ? { ...state, approval: null } : state;
    case 'error':
      return { ...state, error: message.message, running: false };
    default:
      return state;
  }
}

/**
 * Active session panel (T035).
 *
 * Prompt input, the streamed transcript, cancellation, and the approve/reject
 * controls for the pending request (FR-003, FR-005, FR-006, FR-007).
 */

import { useState } from 'react';

import { ApprovalRequest } from '../ApprovalRequest/ApprovalRequest';
import type {
  ApprovalRequest as ApprovalRequestPayload,
  Decision,
  TranscriptEntry,
} from '../../types/protocol';

export interface SessionPanelProps {
  entries: TranscriptEntry[];
  running: boolean;
  approval: ApprovalRequestPayload | null;
  error: string | null;
  disabled?: boolean;
  onSubmit: (prompt: string) => void;
  onCancel: () => void;
  onRespond: (requestId: number, decision: Decision) => void;
}

export function SessionPanel({
  entries,
  running,
  approval,
  error,
  disabled = false,
  onSubmit,
  onCancel,
  onRespond,
}: SessionPanelProps) {
  const [prompt, setPrompt] = useState('');

  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    if (!prompt.trim()) {
      return;
    }
    onSubmit(prompt.trim());
    setPrompt('');
  };

  return (
    <section aria-label="Session">
      <h2>Session</h2>
      {error ? <p role="alert">{error}</p> : null}

      <ol aria-label="Transcript">
        {entries.map((entry) => (
          <li key={entry.order_index} data-entry-type={entry.entry_type}>
            <span>{entry.entry_type}</span>
            <span>{describe(entry)}</span>
            {entry.origin_device_name ? <span>from {entry.origin_device_name}</span> : null}
          </li>
        ))}
      </ol>

      {approval ? <ApprovalRequest request={approval} onRespond={onRespond} /> : null}

      <form onSubmit={submit}>
        <label>
          Prompt
          <textarea
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
            disabled={disabled}
          />
        </label>
        <button type="submit" disabled={disabled || running}>
          Send prompt
        </button>
        <button type="button" onClick={onCancel} disabled={!running}>
          Cancel prompt
        </button>
      </form>
    </section>
  );
}

/** One-line summary of a transcript entry for the list. */
function describe(entry: TranscriptEntry): string {
  const content = entry.content as {
    text?: string;
    message?: string;
    command?: string;
    path?: string;
    operation?: string;
    decision?: string;
  };

  switch (entry.entry_type) {
    case 'prompt':
    case 'response':
      return content.text ?? '';
    case 'command':
      return content.command ?? '';
    case 'file_change':
      return `${content.operation ?? 'change'} ${content.path ?? ''}`.trim();
    case 'approval_decision':
      return content.decision ?? '';
    case 'error':
      return content.message ?? '';
    default:
      return '';
  }
}

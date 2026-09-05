/**
 * Approval dialog (T035 support component).
 *
 * Shows the exact command text or affected paths so the developer can decide
 * (FR-005, FR-018).
 */

import type { ApprovalRequest as ApprovalRequestPayload, Decision } from '../../types/protocol';

export interface ApprovalRequestProps {
  request: ApprovalRequestPayload;
  onRespond: (requestId: number, decision: Decision) => void;
}

export function ApprovalRequest({ request, onRespond }: ApprovalRequestProps) {
  const details = request.details as { command?: string; path?: string; operation?: string };

  return (
    <section aria-label="Approval request">
      <h3>Approval needed</h3>
      <p>{request.request_type === 'command' ? 'Run command' : 'Change file'}</p>
      <pre>{details.command ?? `${details.operation ?? 'change'} ${details.path ?? ''}`}</pre>
      <p>Expires at {request.expires_at}</p>
      <button type="button" onClick={() => onRespond(request.id, 'approved')}>
        Approve
      </button>
      <button type="button" onClick={() => onRespond(request.id, 'denied')}>
        Deny
      </button>
    </section>
  );
}

/**
 * Tests for the desktop session panel and project list (T034, T035).
 */

import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { ProjectList } from '../../src/components/ProjectList/ProjectList';
import { SessionPanel } from '../../src/components/SessionPanel/SessionPanel';
import type { TranscriptEntry } from '../../src/types/protocol';

const entries: TranscriptEntry[] = [
  {
    order_index: 0,
    entry_type: 'prompt',
    origin_device_name: null,
    content: { text: 'add a greeting' },
    created_at: '2026-09-01T10:00:00Z',
  },
  {
    order_index: 1,
    entry_type: 'file_change',
    origin_device_name: 'Phone',
    content: { path: 'greeting.txt', operation: 'create' },
    created_at: '2026-09-01T10:00:01Z',
  },
];

describe('SessionPanel', () => {
  it('renders the streamed transcript with attribution', () => {
    render(
      <SessionPanel
        entries={entries}
        running={false}
        approval={null}
        error={null}
        onSubmit={vi.fn()}
        onCancel={vi.fn()}
        onRespond={vi.fn()}
      />,
    );

    expect(screen.getByText('add a greeting')).toBeDefined();
    expect(screen.getByText('create greeting.txt')).toBeDefined();
    expect(screen.getByText('from Phone')).toBeDefined();
  });

  it('submits a trimmed prompt and clears the input', () => {
    const onSubmit = vi.fn();
    render(
      <SessionPanel
        entries={[]}
        running={false}
        approval={null}
        error={null}
        onSubmit={onSubmit}
        onCancel={vi.fn()}
        onRespond={vi.fn()}
      />,
    );

    const input = screen.getByLabelText('Prompt') as HTMLTextAreaElement;
    fireEvent.change(input, { target: { value: '  run the tests  ' } });
    fireEvent.click(screen.getByText('Send prompt'));

    expect(onSubmit).toHaveBeenCalledWith('run the tests');
    expect(input.value).toBe('');
  });

  it('only allows cancelling while a prompt is running', () => {
    const onCancel = vi.fn();
    const { rerender } = render(
      <SessionPanel
        entries={[]}
        running={false}
        approval={null}
        error={null}
        onSubmit={vi.fn()}
        onCancel={onCancel}
        onRespond={vi.fn()}
      />,
    );
    expect((screen.getByText('Cancel prompt') as HTMLButtonElement).disabled).toBe(true);

    rerender(
      <SessionPanel
        entries={[]}
        running={true}
        approval={null}
        error={null}
        onSubmit={vi.fn()}
        onCancel={onCancel}
        onRespond={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByText('Cancel prompt'));
    expect(onCancel).toHaveBeenCalled();
  });

  it('shows the pending approval with its command and answers it', () => {
    const onRespond = vi.fn();
    render(
      <SessionPanel
        entries={[]}
        running={true}
        approval={{
          id: 9,
          request_type: 'command',
          details: { command: 'npm install' },
          expires_at: '2026-09-01T10:05:00Z',
        }}
        error={null}
        onSubmit={vi.fn()}
        onCancel={vi.fn()}
        onRespond={onRespond}
      />,
    );

    expect(screen.getByText('npm install')).toBeDefined();
    fireEvent.click(screen.getByText('Approve'));
    expect(onRespond).toHaveBeenCalledWith(9, 'approved');

    fireEvent.click(screen.getByText('Deny'));
    expect(onRespond).toHaveBeenCalledWith(9, 'denied');
  });
});

describe('ProjectList', () => {
  it('adds a project and marks unavailable folders', () => {
    const onAdd = vi.fn();
    render(
      <ProjectList
        projects={[
          { id: 1, name: 'demo', path: '/tmp/demo', available: true },
          { id: 2, name: 'gone', path: '/tmp/gone', available: false },
        ]}
        selectedId={1}
        onSelect={vi.fn()}
        onAdd={onAdd}
        onRename={vi.fn()}
        onRemove={vi.fn()}
      />,
    );

    expect(screen.getByText('folder missing')).toBeDefined();
    expect((screen.getByText('gone') as HTMLButtonElement).disabled).toBe(true);

    fireEvent.change(screen.getByLabelText('Project name'), { target: { value: 'new' } });
    fireEvent.change(screen.getByLabelText('Project folder'), { target: { value: '/tmp/new' } });
    fireEvent.click(screen.getByText('Add project'));
    expect(onAdd).toHaveBeenCalledWith('new', '/tmp/new');
  });
});

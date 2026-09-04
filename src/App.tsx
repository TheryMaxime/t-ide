/**
 * Desktop IDE shell.
 *
 * Composes the User Story 1 surfaces: project registration, model provider
 * settings, and the live session panel. Projects and providers are held in
 * component state until the `projects.list` handler lands with User Story 3
 * (T052); the session itself already runs against the backend over WebSocket.
 */

import { useState } from 'react';

import { ProjectList } from './components/ProjectList/ProjectList';
import { SessionPanel } from './components/SessionPanel/SessionPanel';
import { Settings } from './components/Settings/Settings';
import { useSession } from './hooks/useSession';
import type { ModelProvider, Project } from './types/protocol';

export function App() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [providers, setProviders] = useState<ModelProvider[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null);
  const [defaultProviderId, setDefaultProviderId] = useState<number | null>(null);
  const session = useSession();

  const submitPrompt = (prompt: string) => {
    if (session.sessionId !== null) {
      session.sendPrompt(prompt);
    } else if (selectedProjectId !== null && defaultProviderId !== null) {
      session.createSession(selectedProjectId, defaultProviderId, prompt);
    }
  };

  return (
    <main>
      <h1>T-ide</h1>
      <p>Connection: {session.connection}</p>

      <ProjectList
        projects={projects}
        selectedId={selectedProjectId}
        onSelect={setSelectedProjectId}
        onAdd={(name, path) =>
          setProjects((current) => [
            ...current,
            { id: nextId(current), name, path, available: true },
          ])
        }
        onRename={(projectId, name) =>
          setProjects((current) =>
            current.map((project) => (project.id === projectId ? { ...project, name } : project)),
          )
        }
        onRemove={(projectId) => {
          setProjects((current) => current.filter((project) => project.id !== projectId));
          setSelectedProjectId((current) => (current === projectId ? null : current));
        }}
      />

      <Settings
        providers={providers}
        defaultProviderId={defaultProviderId}
        onAddProvider={(provider) =>
          setProviders((current) => {
            const added = { ...provider, id: nextId(current) };
            setDefaultProviderId((selected) => selected ?? added.id);
            return [...current, added];
          })
        }
        onSelectProvider={setDefaultProviderId}
        onRemoveProvider={(providerId) => {
          setProviders((current) => current.filter((provider) => provider.id !== providerId));
          setDefaultProviderId((current) => (current === providerId ? null : current));
        }}
      />

      <SessionPanel
        entries={session.entries}
        running={session.running}
        approval={session.approval}
        error={session.error}
        disabled={selectedProjectId === null || defaultProviderId === null}
        onSubmit={submitPrompt}
        onCancel={session.cancel}
        onRespond={session.respond}
      />
    </main>
  );
}

function nextId(items: { id: number }[]): number {
  return items.reduce((highest, item) => Math.max(highest, item.id), 0) + 1;
}

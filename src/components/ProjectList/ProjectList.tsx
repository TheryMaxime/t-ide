/**
 * Registered project list (T034).
 *
 * Shows every registered project with its availability state and offers
 * add / rename / remove controls (FR-001, FR-002).
 */

import { useState } from 'react';

import type { Project } from '../../types/protocol';

export interface ProjectListProps {
  projects: Project[];
  selectedId: number | null;
  onSelect: (projectId: number) => void;
  onAdd: (name: string, path: string) => void;
  onRename: (projectId: number, name: string) => void;
  onRemove: (projectId: number) => void;
}

export function ProjectList({
  projects,
  selectedId,
  onSelect,
  onAdd,
  onRename,
  onRemove,
}: ProjectListProps) {
  const [name, setName] = useState('');
  const [path, setPath] = useState('');

  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    if (!name.trim() || !path.trim()) {
      return;
    }
    onAdd(name.trim(), path.trim());
    setName('');
    setPath('');
  };

  return (
    <section aria-label="Projects">
      <h2>Projects</h2>
      <ul>
        {projects.map((project) => (
          <li key={project.id}>
            <button
              type="button"
              aria-pressed={project.id === selectedId}
              disabled={!project.available}
              onClick={() => onSelect(project.id)}
            >
              {project.name}
            </button>
            <span>{project.available ? 'available' : 'folder missing'}</span>
            <button
              type="button"
              onClick={() => {
                const renamed = window.prompt('New project name', project.name);
                if (renamed && renamed.trim()) {
                  onRename(project.id, renamed.trim());
                }
              }}
            >
              Rename {project.name}
            </button>
            <button type="button" onClick={() => onRemove(project.id)}>
              Remove {project.name}
            </button>
          </li>
        ))}
      </ul>

      <form onSubmit={submit}>
        <label>
          Project name
          <input value={name} onChange={(event) => setName(event.target.value)} />
        </label>
        <label>
          Project folder
          <input value={path} onChange={(event) => setPath(event.target.value)} />
        </label>
        <button type="submit">Add project</button>
      </form>
    </section>
  );
}

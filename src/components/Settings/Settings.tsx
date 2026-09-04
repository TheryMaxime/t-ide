/**
 * Model provider settings (T036).
 *
 * Registers OpenAI-compatible providers (hosted or local), picks the default,
 * and selects the provider used for the next session. API credentials are
 * typed here and handed straight to the backend keychain: they are never read
 * back into the UI (FR-007c, FR-007d).
 */

import { useState } from 'react';

import type { ModelProvider } from '../../types/protocol';

export interface SettingsProps {
  providers: ModelProvider[];
  defaultProviderId: number | null;
  onAddProvider: (provider: Omit<ModelProvider, 'id'>, apiKey: string) => void;
  onSelectProvider: (providerId: number) => void;
  onRemoveProvider: (providerId: number) => void;
}

export function Settings({
  providers,
  defaultProviderId,
  onAddProvider,
  onSelectProvider,
  onRemoveProvider,
}: SettingsProps) {
  const [name, setName] = useState('');
  const [kind, setKind] = useState<ModelProvider['kind']>('external');
  const [apiBaseUrl, setApiBaseUrl] = useState('');
  const [defaultModel, setDefaultModel] = useState('');
  const [apiKey, setApiKey] = useState('');

  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    if (!name.trim() || !apiBaseUrl.trim() || !defaultModel.trim()) {
      return;
    }
    onAddProvider(
      {
        name: name.trim(),
        kind,
        api_base_url: apiBaseUrl.trim(),
        default_model: defaultModel.trim(),
      },
      apiKey,
    );
    setName('');
    setApiBaseUrl('');
    setDefaultModel('');
    setApiKey('');
  };

  return (
    <section aria-label="Model providers">
      <h2>Model providers</h2>
      <ul>
        {providers.map((provider) => (
          <li key={provider.id}>
            <label>
              <input
                type="radio"
                name="default-provider"
                checked={provider.id === defaultProviderId}
                onChange={() => onSelectProvider(provider.id)}
              />
              {provider.name} ({provider.kind}) — {provider.default_model}
            </label>
            <button type="button" onClick={() => onRemoveProvider(provider.id)}>
              Remove {provider.name}
            </button>
          </li>
        ))}
      </ul>

      <form onSubmit={submit}>
        <label>
          Provider name
          <input value={name} onChange={(event) => setName(event.target.value)} />
        </label>
        <label>
          Provider kind
          <select
            value={kind}
            onChange={(event) => setKind(event.target.value as ModelProvider['kind'])}
          >
            <option value="external">external</option>
            <option value="local">local</option>
          </select>
        </label>
        <label>
          API base URL
          <input value={apiBaseUrl} onChange={(event) => setApiBaseUrl(event.target.value)} />
        </label>
        <label>
          Default model
          <input value={defaultModel} onChange={(event) => setDefaultModel(event.target.value)} />
        </label>
        <label>
          API key
          <input
            type="password"
            value={apiKey}
            onChange={(event) => setApiKey(event.target.value)}
          />
        </label>
        <button type="submit">Add provider</button>
      </form>
    </section>
  );
}

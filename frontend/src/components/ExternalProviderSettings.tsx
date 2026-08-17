'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Switch } from '@/components/ui/switch';

const PROVIDER = 'custom-openai';
const TASK = 'agent_handoff';

type ProviderStatus = {
  provider: string;
  task: string;
  endpoint: string;
  model: string;
  enabled: boolean;
  credentialPresent: boolean;
  credentialMask: string | null;
};

export function ExternalProviderSettings() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8787/v1');
  const [model, setModel] = useState('gcrdings-transfer');
  const [credential, setCredential] = useState('');
  const [status, setStatus] = useState<ProviderStatus | null>(null);
  const [message, setMessage] = useState('Optional provider transfers are disabled by default.');
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<ProviderStatus>('api_get_provider_status', { provider: PROVIDER });
      setStatus(next);
      setEndpoint(next.endpoint);
      setModel(next.model);
    } catch {
      setStatus(null);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const save = async () => {
    setBusy(true);
    try {
      const next = await invoke<ProviderStatus>('api_save_provider_credential', {
        configuration: {
          provider: PROVIDER,
          displayName: 'OpenAI-compatible provider',
          endpoint,
          model,
          task: TASK,
        },
        credential,
      });
      setCredential('');
      setStatus(next);
      setMessage('Credential saved securely in macOS Keychain.');
    } catch {
      setMessage('Credential could not be saved.');
    } finally {
      setBusy(false);
    }
  };

  const testCredential = async () => {
    setBusy(true);
    try {
      await invoke('api_test_provider_credential', { provider: PROVIDER });
      setMessage('Credential is available for an explicit transfer.');
    } catch {
      setMessage('Credential test failed.');
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    try {
      setStatus(await invoke<ProviderStatus>('api_remove_provider_credential', { provider: PROVIDER }));
      setCredential('');
      setMessage('Credential removed and transfer disabled.');
    } catch {
      setMessage('Credential could not be removed.');
    } finally {
      setBusy(false);
    }
  };

  const setEnabled = async (enabled: boolean) => {
    setBusy(true);
    try {
      setStatus(await invoke<ProviderStatus>('api_set_provider_task_enabled', { provider: PROVIDER, enabled }));
      setMessage(enabled ? 'Agent Handoff transfer enabled.' : 'Agent Handoff transfer disabled.');
    } catch {
      setMessage('Task authorization could not be changed.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="mt-6 space-y-4 rounded-lg border border-gray-200 bg-white p-5" aria-labelledby="external-provider-heading">
      <div>
        <h2 id="external-provider-heading" className="text-lg font-semibold">Optional external provider</h2>
        <p className="text-sm text-gray-600">Every transfer requires a separate preview and confirmation.</p>
      </div>
      <label className="block text-sm font-medium">
        Endpoint
        <input className="mt-1 w-full rounded border p-2" value={endpoint} onChange={(event) => setEndpoint(event.target.value)} disabled={busy} />
      </label>
      <label className="block text-sm font-medium">
        Model
        <input className="mt-1 w-full rounded border p-2" value={model} onChange={(event) => setModel(event.target.value)} disabled={busy} />
      </label>
      <label className="block text-sm font-medium">
        Credential
        <input type="password" autoComplete="off" className="mt-1 w-full rounded border p-2" value={credential} onChange={(event) => setCredential(event.target.value)} disabled={busy} />
      </label>
      {status?.credentialPresent && (
        <p className="text-sm">Saved credential: <span aria-label="masked credential">{status.credentialMask}</span></p>
      )}
      <div className="flex flex-wrap gap-2">
        <button type="button" onClick={save} disabled={busy || !credential.trim()} className="rounded bg-blue-600 px-3 py-2 text-sm text-white disabled:opacity-50">
          {status?.credentialPresent ? 'Replace credential' : 'Save credential'}
        </button>
        <button type="button" onClick={testCredential} disabled={busy || !status?.credentialPresent} className="rounded border px-3 py-2 text-sm disabled:opacity-50">Test credential</button>
        <button type="button" onClick={remove} disabled={busy || !status?.credentialPresent} className="rounded border px-3 py-2 text-sm disabled:opacity-50">Remove credential</button>
      </div>
      <div className="flex items-center justify-between gap-4">
        <label htmlFor="enable-agent-handoff" className="text-sm font-medium">Enable Agent Handoff transfer</label>
        <Switch id="enable-agent-handoff" checked={status?.enabled ?? false} onCheckedChange={setEnabled} disabled={busy || !status?.credentialPresent} aria-label="Enable Agent Handoff transfer" />
      </div>
      <p aria-live="polite" className="text-sm text-gray-600">{message}</p>
    </section>
  );
}

import { invoke } from '@tauri-apps/api/core';
import { useCallback, useState } from 'react';
import {
  providerTransferError,
  verifiableRecordErrorCode,
  type ProviderPreview,
  type ProviderTransferOutcome,
} from '@/types/verifiable-record';

const PROVIDER = 'custom-openai';
const PURPOSE = 'Analyze the current Agent Handoff';

export function useExternalProvider(meetingId: string) {
  const [open, setOpen] = useState(false);
  const [preview, setPreview] = useState<ProviderPreview | null>(null);
  const [result, setResult] = useState<ProviderTransferOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);

  const loadPreview = useCallback(async () => {
    setOpen(true);
    setBusy(true);
    setPreview(null);
    setResult(null);
    setError(null);
    setErrorCode(undefined);
    try {
      setPreview(await invoke<ProviderPreview>('api_preview_provider_transfer', {
        meetingId,
        provider: PROVIDER,
        purpose: PURPOSE,
      }));
    } catch (reason) {
      setErrorCode(verifiableRecordErrorCode(reason));
      setError(providerTransferError(reason));
    } finally {
      setBusy(false);
    }
  }, [meetingId]);

  const confirmTransfer = useCallback(async () => {
    if (!preview || busy) return;
    setBusy(true);
    setError(null);
    setErrorCode(undefined);
    try {
      setResult(await invoke<ProviderTransferOutcome>('api_confirm_provider_transfer', {
        previewDigest: preview.previewDigest,
      }));
    } catch (reason) {
      setErrorCode(verifiableRecordErrorCode(reason));
      setError(providerTransferError(reason));
    } finally {
      setBusy(false);
    }
  }, [busy, preview]);

  const close = useCallback(() => {
    if (busy) return;
    setOpen(false);
    setPreview(null);
    setResult(null);
    setError(null);
    setErrorCode(undefined);
  }, [busy]);

  return { open, preview, result, error, errorCode, busy, loadPreview, confirmTransfer, close };
}

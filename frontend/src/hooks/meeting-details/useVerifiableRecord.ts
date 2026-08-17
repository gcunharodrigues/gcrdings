import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { generationMessage, type RecordType, type VerifiableRecord, verifiableRecordErrorCode } from "@/types/verifiable-record";

export function useVerifiableRecord(meetingId: string) {
  const [record, setRecord] = useState<VerifiableRecord | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const generationRef = useRef(0);

  const load = useCallback(async () => {
    const generation = ++generationRef.current;
    setIsLoading(true);
    try {
      const next = await invoke<VerifiableRecord>("api_get_verifiable_record", { meetingId });
      if (generation === generationRef.current) { setRecord(next); setError(null); }
    } catch (reason) {
      if (generation === generationRef.current) setError("Local findings could not be loaded.");
    } finally {
      if (generation === generationRef.current) setIsLoading(false);
    }
  }, [meetingId]);

  useEffect(() => { void load(); return () => { generationRef.current += 1; }; }, [load]);

  const selectType = useCallback(async (recordType: RecordType) => {
    try {
      setRecord(await invoke("api_set_record_type", { meetingId, recordType }));
      setError(null);
    } catch { setError("The record type could not be saved."); }
  }, [meetingId]);

  const generate = useCallback(async () => {
    if (!record || record.generation_status === "processing") return;
    setRecord({ ...record, generation_status: "processing", error_code: null });
    setError(null);
    try {
      setRecord(await invoke("api_generate_verifiable_record", { meetingId, expectedRevision: record.principal_transcript_revision }));
    } catch (reason) {
      const code = verifiableRecordErrorCode(reason);
      if (code === "cancelled") setError(generationMessage("pending", code));
      else setError(code ? generationMessage(code === "stale_revision" ? "stale" : "failed", code) : "Local generation failed.");
      await load();
    }
  }, [load, meetingId, record]);

  const cancel = useCallback(async () => {
    try { await invoke("api_cancel_verifiable_record", { meetingId }); }
    catch { setError("No active local generation was found."); }
  }, [meetingId]);

  return { record, error, isLoading, reload: load, selectType, generate, cancel };
}

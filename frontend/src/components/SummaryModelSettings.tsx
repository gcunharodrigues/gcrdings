import { SummaryLanguageSettings } from "@/components/SummaryLanguageSettings";

export function SummaryModelSettings() {
  return (
    <div className="flex flex-col gap-4">
      <section className="rounded-lg border border-gray-200 bg-white p-6 shadow-sm" aria-labelledby="local-findings-model">
        <h3 id="local-findings-model" className="text-lg font-semibold text-gray-900">Local findings</h3>
        <p className="mt-2 text-sm leading-6 text-gray-600">gcrdings uses Apple Foundation Models on this Mac. There is no model download, server endpoint, or automatic cloud fallback.</p>
      </section>
      <SummaryLanguageSettings />
    </div>
  );
}

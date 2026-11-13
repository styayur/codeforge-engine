import type { Severity, VerificationStatus } from "../types";

export function SeverityPill({ severity }: { severity: Severity }) {
  return <span className={`status-pill severity-${severity}`}>{severity}</span>;
}

export function VerificationPill({ status }: { status: VerificationStatus }) {
  return <span className={`status-pill verify-${status}`}>{status.replace("_", " ")}</span>;
}

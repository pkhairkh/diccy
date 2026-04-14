# Connector Certificate and Secret Rotation Runbook

Status: zero-downtime credential rotation procedure for enterprise connectors.

## 1. Scope

Applies to:
- workflow TLS keypair changes,
- webhook HMAC shared-secret changes,
- connector-side client certificate/credential changes used by callback targets.

## 2. Zero-downtime expectations

- Rotation must preserve callback delivery continuity.
- At least one valid credential set must remain active during cutover.
- Failed rotation must be reversible without data loss.
- DLQ/replay path remains authoritative for recovery:
  - `/interop/hl7/failures`
  - `tools/hl7_dead_letter_replay.py`

## 3. Pre-rotation checklist

1. Verify current health:
   - `/interop/connectors/health` is reachable,
   - callback failure rate is below incident threshold.
2. Confirm secret/cert inventory:
   - old credential ID,
   - new credential ID,
   - rollback credential ID.
3. Confirm rollout and auth ownership:
   - security owner,
   - connector owner,
   - on-call owner.
4. Confirm snapshot/artifact persistence paths are writable.

## 4. Rotation procedure

1. Stage new credentials without disabling current credentials.
2. Apply connector-side and workflow-side config update in controlled order.
3. Validate auth and callback behavior against non-prod traffic.
4. Shift production traffic progressively using connector rollout controls.
5. Confirm deterministic ACK/NACK/error behavior is unchanged.
6. Retire old credentials only after sustained healthy callbacks.

## 5. Failure recovery

If rotation causes callback failures:

1. Immediately revert to rollback credential set.
2. Set connector rollout to `0` for affected alias if needed.
3. Verify callback failures stop growing.
4. Replay eligible dead-letter items after fix.
5. Document root cause and update hardening actions.

## 6. Evidence required per release

- Rotation execution log with timestamps and owners.
- Health and failure dashboard snapshots before/after cutover.
- Replay report if DLQ entries occurred.
- Signed entry in release interoperability evidence index.


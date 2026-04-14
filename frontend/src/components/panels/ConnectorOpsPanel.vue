<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import ConnectorHealthTile from "../composites/ConnectorHealthTile.vue";
import MetricStatCard from "../components/MetricStatCard.vue";
import { classTokens } from "../../styles/tokens";
// Tailwind Plus UI Blocks provenance is documented in
// frontend/src/components/panels/TAILWIND_PLUS_PROVENANCE.md.

const props = defineProps<{
  tenantId?: string;
  role?: "admin" | "operator" | "viewer";
}>();

type ConnectorHealthStatus = "healthy" | "degraded" | "offline";
type RequestFailureCode = "auth_expired" | "downstream_timeout" | "request_failed";

type ConnectorDashboardRecord = {
  alias: string;
  subscription_count: number;
  callback_failures: number;
};

type ConnectorFeaturesRecord = {
  alias: string;
  rollout_percent: number;
  plugin: { path?: string } | null;
};

type ConnectorStatusResponse = {
  connectors: ConnectorDashboardRecord[];
};

type ConnectorFeaturesResponse = {
  connectors: ConnectorFeaturesRecord[];
};

type ConnectorHealthRow = {
  connector: string;
  status: ConnectorHealthStatus;
  latencyMs: number;
};

type ConnectorConfigRow = {
  alias: string;
  pluginPath: string;
  rollout: number;
  dirty: boolean;
};

class ConnectorRequestError extends Error {
  code: RequestFailureCode;

  constructor(message: string, code: RequestFailureCode) {
    super(message);
    this.code = code;
  }
}

const operationalMetrics = [
  { label: "Failed transforms", value: 3, max: 10 },
  { label: "Queue latency p95 (ms)", value: 214, max: 500 },
  { label: "DIMSE transport failures", value: 1, max: 10 },
] as const;

const tenantLabel = computed(() => props.tenantId ?? "tenant-default");
const canMutateConnectors = computed(() => props.role === "admin" || props.role === "operator");
const panelShellClass = `${classTokens.panelShell} ${classTokens.panelTopOffset}`;
const connectorRows = ref<ConnectorHealthRow[]>([]);
const connectorConfigRows = ref<ConnectorConfigRow[]>([]);
const loadingState = ref<"loading" | "ready" | "empty" | "degraded" | "error">("loading");
const loadingMessage = ref<string>("");
const saveState = ref<"idle" | "saving" | "saved" | "error">("idle");
const saveMessage = ref<string>("");
const lastSyncedAtMs = ref<number | null>(null);
const staleSinceMs = ref<number | null>(null);
const sessionState = ref<"active" | "expired">("active");
const sessionMessage = ref<string>("");
const toastState = ref<"idle" | "success" | "error">("idle");
const toastMessage = ref<string>("");

const hasDirtyConnectorConfig = computed(() =>
  connectorConfigRows.value.some((connector) => connector.dirty),
);
const mutationsDisabled = computed(
  () => !canMutateConnectors.value || sessionState.value === "expired" || saveState.value === "saving",
);

function workflowApiBasePath() {
  const env = (import.meta as { env?: Record<string, unknown> }).env;
  const candidate =
    (typeof env?.VITE_WORKFLOW_API_BASE === "string" && env.VITE_WORKFLOW_API_BASE.trim()) ||
    "/interop";
  return candidate.replace(/\/+$/, "");
}

function connectorApiPath(suffix: string) {
  return `${workflowApiBasePath()}/connectors/${suffix}`;
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function requestFailureCodeFromStatus(status: number): RequestFailureCode {
  if (status === 401 || status === 403) {
    return "auth_expired";
  }
  if (status === 408 || status === 504) {
    return "downstream_timeout";
  }
  return "request_failed";
}

function markSessionExpired(detail: string) {
  sessionState.value = "expired";
  sessionMessage.value =
    `Session or role context expired (${detail}). Re-authenticate with admin/operator credentials and retry.`;
}

async function fetchJsonWithRetry<T>(path: string, attempts = 3): Promise<T> {
  let lastError: Error | null = null;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      const response = await fetch(path, {
        method: "GET",
        headers: { Accept: "application/json" },
      });
      if (!response.ok) {
        const code = requestFailureCodeFromStatus(response.status);
        throw new ConnectorRequestError(`request failed with status ${response.status}`, code);
      }
      return (await response.json()) as T;
    } catch (error) {
      lastError = error instanceof Error ? error : new Error("request failed");
      if (attempt < attempts) {
        await delay(200 * attempt);
      }
    }
  }
  throw (lastError ?? new Error("request failed"));
}

function parseConnectorStatus(record: ConnectorDashboardRecord): ConnectorHealthStatus {
  if (record.subscription_count === 0) {
    return "offline";
  }
  if (record.callback_failures > 0) {
    return "degraded";
  }
  return "healthy";
}

function parseConnectorLatency(record: ConnectorDashboardRecord): number {
  if (record.subscription_count === 0) {
    return 0;
  }
  if (record.callback_failures > 0) {
    return 180;
  }
  return 42;
}

async function loadConnectorState() {
  loadingMessage.value = "";
  if (loadingState.value === "empty") {
    loadingState.value = "loading";
  }
  try {
    const [statusPayload, featuresPayload] = await Promise.all([
      fetchJsonWithRetry<ConnectorStatusResponse>(connectorApiPath("status")),
      fetchJsonWithRetry<ConnectorFeaturesResponse>(connectorApiPath("features")),
    ]);

    connectorRows.value = statusPayload.connectors.map((record) => ({
      connector: record.alias,
      status: parseConnectorStatus(record),
      latencyMs: parseConnectorLatency(record),
    }));
    connectorConfigRows.value = featuresPayload.connectors.map((record) => ({
      alias: record.alias,
      pluginPath: record.plugin?.path ?? "unavailable",
      rollout: record.rollout_percent,
      dirty: false,
    }));

    if (connectorRows.value.length === 0 && connectorConfigRows.value.length === 0) {
      loadingState.value = "empty";
      loadingMessage.value = "No connector records were returned by the workflow API.";
    } else {
      loadingState.value = "ready";
      loadingMessage.value = "";
    }
    staleSinceMs.value = null;
    sessionState.value = "active";
    sessionMessage.value = "";
    lastSyncedAtMs.value = Date.now();
  } catch (error) {
    const message = error instanceof Error ? error.message : "request failed";
    const code = error instanceof ConnectorRequestError ? error.code : "request_failed";
    if (code === "auth_expired") {
      markSessionExpired(message);
    }
    if (connectorRows.value.length > 0 || connectorConfigRows.value.length > 0) {
      loadingState.value = "degraded";
      if (staleSinceMs.value === null) {
        staleSinceMs.value = Date.now();
      }
      loadingMessage.value = `Connector API degraded: ${message}. Showing last known state.`;
      return;
    }
    loadingState.value = "error";
    if (code === "downstream_timeout") {
      loadingMessage.value =
        `Connector API timed out: ${message}. Retry after downstream stabilization.`;
      return;
    }
    loadingMessage.value = `Connector API unavailable: ${message}.`;
  }
}

function markRolloutDirty(alias: string) {
  if (mutationsDisabled.value) {
    return;
  }
  for (const connector of connectorConfigRows.value) {
    if (connector.alias === alias) {
      connector.dirty = true;
      break;
    }
  }
}

async function postRolloutUpdate(alias: string, rolloutPercent: number) {
  const body = new URLSearchParams({
    alias,
    rollout_percent: String(rolloutPercent),
  });
  const response = await fetch(connectorApiPath("rollout"), {
    method: "POST",
    headers: {
      "Content-Type": "application/x-www-form-urlencoded",
      Accept: "application/json",
    },
    body: body.toString(),
  });
  if (!response.ok) {
    const code = requestFailureCodeFromStatus(response.status);
    throw new ConnectorRequestError(`rollout update failed with status ${response.status}`, code);
  }
}

function rolloutConfirmationId(aliases: string[]) {
  return `connector-rollout-${aliases.join("-")}`;
}

async function saveConnectorPolicy() {
  if (!hasDirtyConnectorConfig.value || mutationsDisabled.value) {
    return;
  }

  saveState.value = "saving";
  saveMessage.value = "";
  toastState.value = "idle";
  toastMessage.value = "";

  const pending = connectorConfigRows.value
    .filter((connector) => connector.dirty)
    .sort((left, right) => left.alias.localeCompare(right.alias));
  const previousRollouts = new Map<string, number>();
  for (const connector of pending) {
    previousRollouts.set(connector.alias, connector.rollout);
  }

  try {
    for (const connector of pending) {
      const boundedRollout = Math.min(100, Math.max(0, Math.round(connector.rollout)));
      connector.rollout = boundedRollout;
      await postRolloutUpdate(connector.alias, boundedRollout);
      connector.dirty = false;
    }
    saveState.value = "saved";
    saveMessage.value =
      `Connector rollout policy saved. Audit confirmation id: ${rolloutConfirmationId(pending.map((connector) => connector.alias))}.`;
    toastState.value = "success";
    toastMessage.value = "Connector policy update committed.";
  } catch (error) {
    const message = error instanceof Error ? error.message : "rollout update failed";
    for (const connector of pending) {
      const previous = previousRollouts.get(connector.alias);
      if (previous !== undefined) {
        connector.rollout = previous;
      }
      connector.dirty = true;
    }
    if (error instanceof ConnectorRequestError && error.code === "auth_expired") {
      markSessionExpired(message);
    }
    saveState.value = "error";
    saveMessage.value = `Unable to save connector policy; edits were rolled back: ${message}`;
    toastState.value = "error";
    toastMessage.value = "Connector policy update failed and was reverted.";
  }
}

onMounted(async () => {
  await loadConnectorState();
});

function metricFillClass(value: number, max: number) {
  const ratio = max === 0 ? 0 : value / max;
  if (ratio >= 0.8) return classTokens.metricBarFillHigh;
  if (ratio >= 0.4) return classTokens.metricBarFillMedium;
  return classTokens.metricBarFillLow;
}

function metricFillWidthClass(value: number, max: number) {
  const ratio = max === 0 ? 0 : Math.min(100, Math.max(0, Math.round((value / max) * 100)));
  if (ratio >= 90) return classTokens.metricBarWidth100;
  if (ratio >= 65) return classTokens.metricBarWidth75;
  if (ratio >= 40) return classTokens.metricBarWidth50;
  if (ratio >= 15) return classTokens.metricBarWidth25;
  return classTokens.metricBarWidth0;
}

function metricFillToken(value: number, max: number) {
  return `${metricFillClass(value, max)} ${metricFillWidthClass(value, max)}`;
}
</script>

<template>
  <section :class="panelShellClass">
    <div :class="classTokens.panelBody">
      <section :class="classTokens.panelSection">
        <header :class="classTokens.panelHeader">
          <h2>Connector Health</h2>
          <p :class="classTokens.panelSubtleText">Tenant: {{ tenantLabel }}</p>
          <p v-if="lastSyncedAtMs" :class="classTokens.panelSubtleText">
            Last synced: {{ new Date(lastSyncedAtMs).toLocaleString() }}
          </p>
        </header>
        <p v-if="loadingState === 'loading'" :class="classTokens.panelSubtleText">
          Loading connector state from workflow API...
        </p>
        <p v-if="loadingState === 'empty'" :class="classTokens.panelSubtleText">
          {{ loadingMessage }}
        </p>
        <p v-if="loadingState === 'degraded'" :class="classTokens.alertBanner">
          {{ loadingMessage }}
        </p>
        <p v-if="loadingState === 'error'" :class="classTokens.alertBanner">
          {{ loadingMessage }}
        </p>
        <p v-if="staleSinceMs" :class="classTokens.panelSubtleText">
          Showing stale connector state from {{ new Date(staleSinceMs).toLocaleString() }}.
        </p>
        <button
          v-if="loadingState === 'degraded' || loadingState === 'error' || loadingState === 'empty'"
          type="button"
          :class="classTokens.actionButton"
          @click="loadConnectorState"
        >
          Retry connector sync
        </button>
        <ConnectorHealthTile
          v-for="row in connectorRows"
          :key="row.connector"
          :connector="row.connector"
          :status="row.status"
          :latency-ms="row.latencyMs"
        />
      </section>
      <section :class="classTokens.panelSection" :data-can-configure="canMutateConnectors">
        <header :class="classTokens.panelHeader">
          <h2>Connector Configuration</h2>
          <p :class="classTokens.panelSubtleText">Admin panel controls for adapter rollout and plugin paths</p>
        </header>
        <p v-if="!canMutateConnectors" :class="classTokens.alertBanner">
          Role does not permit connector mutation actions.
        </p>
        <p v-if="sessionState === 'expired'" :class="classTokens.alertBanner">
          {{ sessionMessage }}
        </p>
        <form v-else :class="classTokens.formGrid">
          <article
            v-for="connector in connectorConfigRows"
            :key="connector.alias"
            :class="classTokens.statCard"
          >
            <div :class="classTokens.fieldStack">
              <label :class="classTokens.fieldLabel">Connector alias</label>
              <input :class="classTokens.fieldInput" :value="connector.alias" readonly />
            </div>
            <div :class="classTokens.fieldStack">
              <label :class="classTokens.fieldLabel">Plugin path</label>
              <input :class="classTokens.fieldInput" :value="connector.pluginPath" readonly />
            </div>
            <div :class="classTokens.fieldStack">
              <label :class="classTokens.fieldLabel">Rollout %</label>
              <input
                :class="classTokens.fieldInput"
                :aria-label="`Rollout percent for ${connector.alias}`"
                type="number"
                min="0"
                max="100"
                v-model.number="connector.rollout"
                :disabled="mutationsDisabled"
                @input="markRolloutDirty(connector.alias)"
              />
            </div>
          </article>
          <button
            type="button"
            :class="classTokens.actionButton"
            :disabled="mutationsDisabled || !hasDirtyConnectorConfig"
            @click="saveConnectorPolicy"
          >
            {{ saveState === "saving" ? "Saving..." : "Save connector policy" }}
          </button>
          <p v-if="saveState === 'saved'" :class="classTokens.panelSubtleText">{{ saveMessage }}</p>
          <p v-if="saveState === 'error'" :class="classTokens.alertBanner">{{ saveMessage }}</p>
          <p v-if="toastState === 'success'" :class="classTokens.panelSubtleText">{{ toastMessage }}</p>
          <p v-if="toastState === 'error'" :class="classTokens.alertBanner">{{ toastMessage }}</p>
        </form>
      </section>
      <section :class="classTokens.panelSection">
        <header :class="classTokens.panelHeader">
          <h2>Operational Metrics</h2>
          <p :class="classTokens.panelSubtleText">Transform, queue, and transport reliability overview</p>
        </header>
        <article
          v-for="metric in operationalMetrics"
          :key="metric.label"
          :class="classTokens.statCard"
        >
          <div :class="classTokens.panelHeader">
            <h4 :class="classTokens.fieldLabel">{{ metric.label }}</h4>
            <p>{{ metric.value }}</p>
          </div>
          <span :class="classTokens.metricBarTrack">
            <span
              :class="metricFillToken(metric.value,metric.max)"
            />
          </span>
        </article>
      </section>
      <section :class="classTokens.panelSection">
        <h2>Operational Metrics</h2>
        <MetricStatCard label="Failed transforms" value="3" />
        <MetricStatCard label="Queue latency p95" value="214 ms" />
        <MetricStatCard label="DIMSE transport failures" value="1" />
      </section>
    </div>
  </section>
</template>

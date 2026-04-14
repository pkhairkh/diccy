from __future__ import annotations

import pathlib
import unittest


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]


class DeployManifestSecurityBaselineTests(unittest.TestCase):
    def test_backend_compose_uses_secure_defaults(self) -> None:
        content = (REPO_ROOT / "deploy/docker-compose.backend-services.yml").read_text(encoding="utf-8")
        self.assertNotIn("allow_all", content)
        self.assertNotIn("allow_insecure", content)
        self.assertIn("DICOM_WEB_AUTH_MODE: ${DICOM_WEB_AUTH_MODE:-deny_all}", content)
        self.assertIn("DICOM_WEB_TLS_POLICY: ${DICOM_WEB_TLS_POLICY:-require_tls}", content)
        self.assertIn("DICOM_WEB_TRANSPORT_SECURITY: ${DICOM_WEB_TRANSPORT_SECURITY:-tls}", content)
        self.assertIn("DICOM_WORKFLOW_AUTH_MODE: ${DICOM_WORKFLOW_AUTH_MODE:-token}", content)
        self.assertIn("DICOM_WORKFLOW_SECRET_DIR: ${DICOM_WORKFLOW_SECRET_DIR:-/run/secrets/workflow}", content)
        self.assertIn("DICOM_WORKFLOW_AUTH_TOKEN_PATH: ${DICOM_WORKFLOW_AUTH_TOKEN_PATH:-auth_token}", content)
        self.assertNotIn("DICOM_WORKFLOW_AUTH_TOKEN:", content)

    def test_workstation_compose_uses_secure_defaults(self) -> None:
        content = (REPO_ROOT / "deploy/docker-compose.workstation.yml").read_text(encoding="utf-8")
        self.assertNotIn("allow_all", content)
        self.assertNotIn("allow_insecure", content)
        self.assertIn("DICOM_WEB_AUTH_MODE: ${DICOM_WEB_AUTH_MODE:-deny_all}", content)
        self.assertIn("DICOM_WORKFLOW_AUTH_MODE: ${DICOM_WORKFLOW_AUTH_MODE:-token}", content)
        self.assertIn("DICOM_WORKFLOW_SECRET_DIR: ${DICOM_WORKFLOW_SECRET_DIR:-/run/secrets/workflow}", content)
        self.assertIn("DICOM_WORKFLOW_AUTH_TOKEN_PATH: ${DICOM_WORKFLOW_AUTH_TOKEN_PATH:-auth_token}", content)
        self.assertNotIn("DICOM_WORKFLOW_AUTH_TOKEN:", content)

    def test_env_examples_publish_secure_transport_material(self) -> None:
        web_env = (REPO_ROOT / "deploy/dicom-web-server.env.example").read_text(encoding="utf-8")
        workflow_env = (REPO_ROOT / "deploy/dicom-workflow-server.env.example").read_text(encoding="utf-8")
        self.assertIn("DICOM_WEB_TLS_POLICY=require_tls", web_env)
        self.assertIn("DICOM_WEB_TRANSPORT_SECURITY=tls", web_env)
        self.assertIn("DICOM_WORKFLOW_TRANSPORT_SECURITY=tls", workflow_env)
        self.assertIn("DICOM_WORKFLOW_AUTH_MODE=token", workflow_env)
        self.assertIn("DICOM_WORKFLOW_SECRET_DIR=", workflow_env)
        self.assertIn("DICOM_WORKFLOW_AUTH_TOKEN_PATH=", workflow_env)
        self.assertIn("DICOM_WORKFLOW_TLS_CERT_PATH=", workflow_env)
        self.assertIn("DICOM_WORKFLOW_TLS_KEY_PATH=", workflow_env)

    def test_enterprise_k8s_manifest_uses_supported_secure_contract(self) -> None:
        content = (REPO_ROOT / "deploy/k8s/dicom-enterprise-runtime.yaml").read_text(encoding="utf-8")
        self.assertIn("value: require_tls", content)
        self.assertIn("name: DICOM_WEB_TRANSPORT_SECURITY", content)
        self.assertIn("name: DICOM_WEB_WORKERS", content)
        self.assertNotIn("DICOM_WEB_MAX_CONCURRENT_REQUESTS", content)
        self.assertIn("runAsNonRoot: true", content)
        self.assertIn("runAsUser: 10001", content)
        self.assertIn("runAsGroup: 10001", content)
        self.assertIn("fsGroup: 10001", content)
        self.assertIn("allowPrivilegeEscalation: false", content)
        self.assertIn("readOnlyRootFilesystem: true", content)
        self.assertIn("type: RuntimeDefault", content)
        self.assertIn("path: /readyz", content)
        self.assertIn("path: /healthz", content)
        self.assertIn("startupProbe:", content)
        self.assertIn("terminationGracePeriodSeconds: 30", content)
        self.assertIn("priorityClassName: dicom-critical-services", content)
        self.assertIn("priorityClassName: dicom-interop-services", content)
        self.assertIn("name: runtime-state", content)
        self.assertIn("persistentVolumeClaim:", content)
        self.assertIn("claimName: dicom-web-runtime-state-pvc", content)
        self.assertIn("claimName: dicom-workflow-runtime-state-pvc", content)
        self.assertIn("claimName: dicom-dimse-runtime-state-pvc", content)
        self.assertIn("replicas: 1", content)
        self.assertIn("name: dicom-dimse-service", content)
        self.assertIn("port: 18112", content)
        self.assertIn("name: DICOM_WORKFLOW_TLS_CERT_PATH", content)
        self.assertIn("name: DICOM_WORKFLOW_TLS_KEY_PATH", content)
        self.assertIn("name: DICOM_WORKFLOW_SECRET_DIR", content)
        self.assertIn("name: DICOM_WORKFLOW_AUTH_TOKEN_PATH", content)
        self.assertNotIn("name: DICOM_WORKFLOW_AUTH_TOKEN\n", content)
        self.assertIn("projected:", content)
        self.assertIn("name: workflow-secrets", content)
        self.assertIn("name: workflow-tls", content)
        self.assertIn("name: workflow-auth", content)
        self.assertIn("name: DICOM_DIMSE_TLS_CERT_PATH", content)
        self.assertIn("name: DICOM_DIMSE_TLS_KEY_PATH", content)
        self.assertIn("secretName: dimse-tls", content)
        self.assertIn("automountServiceAccountToken: false", content)
        self.assertIn("podAntiAffinity:", content)
        self.assertIn("topologySpreadConstraints:", content)

    def test_core_k8s_manifest_uses_non_root_runtime_controls(self) -> None:
        content = (REPO_ROOT / "deploy/k8s/dicom-core-runtime.yaml").read_text(encoding="utf-8")
        self.assertIn("runAsNonRoot: true", content)
        self.assertIn("runAsUser: 10001", content)
        self.assertIn("runAsGroup: 10001", content)
        self.assertIn("fsGroup: 10001", content)
        self.assertIn("allowPrivilegeEscalation: false", content)
        self.assertIn("readOnlyRootFilesystem: true", content)
        self.assertIn("type: RuntimeDefault", content)
        self.assertIn("automountServiceAccountToken: false", content)
        self.assertIn("podAntiAffinity:", content)
        self.assertIn("topologySpreadConstraints:", content)
        self.assertIn("priorityClassName: dicom-core-services", content)
        self.assertIn("name: runtime-state", content)
        self.assertIn("emptyDir: {}", content)

    def test_enterprise_network_policy_baseline_exists(self) -> None:
        content = (REPO_ROOT / "deploy/k8s/dicom-enterprise-network-policy.yaml").read_text(encoding="utf-8")
        self.assertIn("kind: NetworkPolicy", content)
        self.assertIn("name: dicom-default-deny-ingress", content)
        self.assertIn("name: dicom-web-allow-ingress", content)
        self.assertIn("name: dicom-workflow-allow-ingress", content)
        self.assertIn("name: dicom-dimse-allow-ingress", content)
        self.assertIn("port: 8080", content)
        self.assertIn("port: 8082", content)
        self.assertIn("port: 11112", content)

    def test_enterprise_pdb_baseline_exists(self) -> None:
        content = (REPO_ROOT / "deploy/k8s/dicom-enterprise-pdb.yaml").read_text(encoding="utf-8")
        self.assertIn("kind: PodDisruptionBudget", content)
        self.assertIn("name: dicom-web-runtime-pdb", content)
        self.assertIn("name: dicom-workflow-runtime-pdb", content)
        self.assertIn("name: dicom-dimse-runtime-pdb", content)
        self.assertIn("minAvailable: 1", content)

    def test_priority_class_baseline_exists(self) -> None:
        content = (REPO_ROOT / "deploy/k8s/dicom-enterprise-priority-classes.yaml").read_text(encoding="utf-8")
        self.assertIn("kind: PriorityClass", content)
        self.assertIn("name: dicom-critical-services", content)
        self.assertIn("name: dicom-interop-services", content)
        self.assertIn("name: dicom-core-services", content)
        self.assertIn("value: 100000", content)

    def test_namespace_and_quota_baseline_exists(self) -> None:
        content = (REPO_ROOT / "deploy/k8s/dicom-enterprise-namespace-and-quotas.yaml").read_text(encoding="utf-8")
        self.assertIn("kind: Namespace", content)
        self.assertIn("name: dicom-enterprise", content)
        self.assertIn("pod-security.kubernetes.io/enforce: restricted", content)
        self.assertIn("kind: LimitRange", content)
        self.assertIn("kind: ResourceQuota", content)

    def test_enterprise_state_pvc_baseline_exists(self) -> None:
        content = (REPO_ROOT / "deploy/k8s/dicom-enterprise-state-pvc.yaml").read_text(encoding="utf-8")
        self.assertIn("kind: PersistentVolumeClaim", content)
        self.assertIn("name: dicom-web-runtime-state-pvc", content)
        self.assertIn("name: dicom-workflow-runtime-state-pvc", content)
        self.assertIn("name: dicom-dimse-runtime-state-pvc", content)
        self.assertIn("namespace: dicom-enterprise", content)


if __name__ == "__main__":
    unittest.main()

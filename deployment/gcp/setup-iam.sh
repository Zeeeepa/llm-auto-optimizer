#!/bin/bash
# LLM-Auto-Optimizer IAM Setup
#
# Creates the service account and assigns minimum required permissions.
# Run this ONCE before first deployment.
#
# Usage: ./setup-iam.sh [PROJECT_ID]

set -euo pipefail

PROJECT_ID="${1:-$(gcloud config get-value project)}"
REGION="us-central1"
SERVICE_ACCOUNT_NAME="llm-auto-optimizer-sa"
SERVICE_ACCOUNT_EMAIL="${SERVICE_ACCOUNT_NAME}@${PROJECT_ID}.iam.gserviceaccount.com"

echo "=== LLM-Auto-Optimizer IAM Setup ==="
echo "Project: ${PROJECT_ID}"
echo "Service Account: ${SERVICE_ACCOUNT_EMAIL}"
echo ""

# ============================================================================
# 1. Create Service Account
# ============================================================================
echo "Creating service account..."
gcloud iam service-accounts create ${SERVICE_ACCOUNT_NAME} \
    --project="${PROJECT_ID}" \
    --description="LLM-Auto-Optimizer Cloud Run service account" \
    --display-name="LLM-Auto-Optimizer Service" \
    2>/dev/null || echo "Service account already exists"

# ============================================================================
# 2. Grant Minimum Required Roles (Least Privilege)
# ============================================================================
echo "Granting IAM roles..."

# Cloud Run Invoker - Allow internal services to invoke
gcloud projects add-iam-policy-binding ${PROJECT_ID} \
    --member="serviceAccount:${SERVICE_ACCOUNT_EMAIL}" \
    --role="roles/run.invoker" \
    --condition=None \
    --quiet

# Secret Manager Access - Read secrets only
gcloud projects add-iam-policy-binding ${PROJECT_ID} \
    --member="serviceAccount:${SERVICE_ACCOUNT_EMAIL}" \
    --role="roles/secretmanager.secretAccessor" \
    --condition=None \
    --quiet

# Cloud Trace Agent - Write traces
gcloud projects add-iam-policy-binding ${PROJECT_ID} \
    --member="serviceAccount:${SERVICE_ACCOUNT_EMAIL}" \
    --role="roles/cloudtrace.agent" \
    --condition=None \
    --quiet

# Logging Writer - Write logs
gcloud projects add-iam-policy-binding ${PROJECT_ID} \
    --member="serviceAccount:${SERVICE_ACCOUNT_EMAIL}" \
    --role="roles/logging.logWriter" \
    --condition=None \
    --quiet

# Monitoring Metric Writer - Write metrics
gcloud projects add-iam-policy-binding ${PROJECT_ID} \
    --member="serviceAccount:${SERVICE_ACCOUNT_EMAIL}" \
    --role="roles/monitoring.metricWriter" \
    --condition=None \
    --quiet

echo ""
echo "✓ IAM setup complete"

# ============================================================================
# 3. Create Secrets (if not exists)
# ============================================================================
echo ""
echo "Creating secrets placeholders..."

# RUVECTOR_API_KEY
gcloud secrets create RUVECTOR_API_KEY \
    --project="${PROJECT_ID}" \
    --replication-policy="automatic" \
    2>/dev/null || echo "RUVECTOR_API_KEY secret already exists"

# RUVECTOR_SERVICE_URL
gcloud secrets create RUVECTOR_SERVICE_URL \
    --project="${PROJECT_ID}" \
    --replication-policy="automatic" \
    2>/dev/null || echo "RUVECTOR_SERVICE_URL secret already exists"

# TELEMETRY_ENDPOINT
gcloud secrets create TELEMETRY_ENDPOINT \
    --project="${PROJECT_ID}" \
    --replication-policy="automatic" \
    2>/dev/null || echo "TELEMETRY_ENDPOINT secret already exists"

echo ""
echo "=== Setup Complete ==="
echo ""
echo "Next steps:"
echo "1. Set secret values:"
echo "   echo -n 'your-api-key' | gcloud secrets versions add RUVECTOR_API_KEY --data-file=-"
echo "   echo -n 'https://ruvector-service-url' | gcloud secrets versions add RUVECTOR_SERVICE_URL --data-file=-"
echo "   echo -n 'https://observatory-url' | gcloud secrets versions add TELEMETRY_ENDPOINT --data-file=-"
echo ""
echo "2. Deploy service:"
echo "   gcloud builds submit --config cloudbuild.yaml"
echo ""

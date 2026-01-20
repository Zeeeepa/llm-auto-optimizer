# LLM-Auto-Optimizer - Google Cloud Run Deployment
# Unified stateless service for optimization agents
#
# Build: docker build -t gcr.io/agentics-dev/llm-auto-optimizer:latest .
# Run:   docker run -p 8080:8080 --env-file .env gcr.io/agentics-dev/llm-auto-optimizer:latest

# ============================================================================
# Stage 1: Build
# ============================================================================
FROM node:20-slim AS builder

WORKDIR /app

# Copy package files
COPY package.json package-lock.json* ./

# Install dependencies
RUN npm ci --only=production=false

# Copy source
COPY tsconfig.json ./
COPY src/ ./src/

# Build TypeScript
RUN npm run build

# ============================================================================
# Stage 2: Production
# ============================================================================
FROM node:20-slim AS production

# Security: Run as non-root user
RUN groupadd -r optimizer && useradd -r -g optimizer optimizer

WORKDIR /app

# Copy package files
COPY package.json package-lock.json* ./

# Install production dependencies only
RUN npm ci --only=production && \
    npm cache clean --force

# Copy built artifacts
COPY --from=builder /app/dist ./dist

# Set ownership
RUN chown -R optimizer:optimizer /app

# Switch to non-root user
USER optimizer

# Expose Cloud Run port
EXPOSE 8080

# Set environment defaults
ENV NODE_ENV=production \
    PORT=8080 \
    SERVICE_NAME=llm-auto-optimizer \
    SERVICE_VERSION=1.0.0

# Health check (Cloud Run handles this, but good for local testing)
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD node -e "require('http').get('http://localhost:8080/health', (r) => process.exit(r.statusCode === 200 ? 0 : 1))"

# Run the server
CMD ["node", "dist/server.js"]

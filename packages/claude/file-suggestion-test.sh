#!/bin/bash
# DX test suite for file-suggestion.sh
# Self-contained: builds a fixture project + workspace siblings + fake $HOME
# under mktemp -d, runs the suite against them, cleans up on exit.
# No machine-specific dependencies — runs identically on any macOS box.
#
# Usage:
#   ./file-suggestion-test.sh                    # tests sibling file-suggestion.sh
#   ./file-suggestion-test.sh /path/to/script.sh # tests a specific script

# ---- Locate script under test --------------------------------------------
SOURCE="${BASH_SOURCE[0]}"
while [ -L "$SOURCE" ]; do
  TARGET="$(readlink "$SOURCE")"
  case "$TARGET" in
    /*) SOURCE="$TARGET" ;;
    *) SOURCE="$(cd -P "$(dirname "$SOURCE")" && pwd)/$TARGET" ;;
  esac
done
TEST_DIR="$(cd -P "$(dirname "$SOURCE")" && pwd)"

SCRIPT="${1:-$TEST_DIR/file-suggestion.sh}"
[[ ! -f "$SCRIPT" ]] && { echo "Script not found: $SCRIPT"; exit 1; }

# ---- Fixture setup -------------------------------------------------------
FIX="$(mktemp -d -t fsg-test-XXXXXX)"
FAKE_HOME="$FIX/home"
WS="$FAKE_HOME/Developer"
PROJ="$WS/proj"
SIBLING="$WS/sibling"
REFS="$WS/references"

REAL_HOME="$HOME"

cleanup() {
  HOME="$REAL_HOME"
  rm -rf "$FIX"
}
trap cleanup EXIT

mkdir -p "$FAKE_HOME/.claude/plans"
mkdir -p "$WS" "$SIBLING" "$REFS/mago/src/crates" "$REFS/agentation" "$REFS/excalidraw"
touch "$SIBLING/README.md"
mkdir -p "$SIBLING/src"
touch "$SIBLING/src/index.js"
touch "$REFS/mago/src/main.rs"
touch "$REFS/agentation/README.md"
# History file under fake .claude
for i in 1 2 3 4 5 6 7 8 9 10 11 12; do
  touch "$FAKE_HOME/.claude/setting$i.json"
done
touch "$FAKE_HOME/.claude/history.jsonl"

# ---- Build the project fixture ------------------------------------------
mkdir -p "$PROJ"
cd "$PROJ"

git init -q
git config user.email t@t.t
git config user.name t
git config commit.gpgsign false

# Top-level files
touch artisan README.md package.json bun.lock vitest.config.ts vite.config.js tsconfig.json
touch .gitignore .editorconfig .env
touch media.test.ts

# .claude/ project-local config (depth 2+)
mkdir -p .claude/agents/frontend .claude/agents/architect .claude/rules
touch .claude/Claude.md
touch .claude/agents/frontend/notes.md
touch .claude/agents/architect/notes.md
touch .claude/rules/coding.md

# Standard Laravel-ish directories
mkdir -p routes tests/api config database/migrations scripts resources/views trellis/group_vars
touch routes/api.php
touch tests/api/auth.test.ts tests/api/media.test.ts tests/api/user.test.ts \
      tests/api/checkout.test.ts tests/api/subscription.test.ts tests/api/session.test.ts
touch config/database.php config/mail.php config/auth.php config/app.php \
      config/cache.php config/queue.php config/services.php
touch database/migrations/0001_create_users.php database/migrations/0007_create_contacts.php \
      database/migrations/0014_add_subscriptions.php
touch scripts/deploy.sh
touch resources/views/layout.blade.php
touch trellis/group_vars/.gitkeep

# admin/ — give it >5 children for "browse admin/" test
mkdir -p admin/components admin/pages admin/layouts admin/api admin/styles admin/utils
touch admin/components/PagesController.php admin/components/createApi.ts
touch admin/pages/dashboard.tsx admin/layouts/main.tsx admin/api/client.ts \
      admin/styles/theme.css admin/utils/format.ts

# app/ — main code with depth + naming variety
mkdir -p app/Models app/Controllers app/Services app/Providers \
         app/Auth app/Sessions \
         app/Subscription \
         app/Tenant/Store/Events
touch app/Models/User.php app/Models/UserModel.php app/Models/Media.php app/Models/Subscription.php
touch app/Controllers/MediaController.php app/Controllers/AuthController.php \
      app/Controllers/PagesController.php app/Controllers/SubscriptionController.php
touch app/Services/MediaService.php app/Services/AuthService.php app/Services/TenantService.php
touch app/Services/BaseConfigurator.php app/Services/BaseJob.php \
      app/Services/SchemaRegistryService.php app/Services/CapabilityService.php \
      app/Services/TenantBootstrapper.php app/Services/DynamicDataProvider.php
touch app/Providers/TenantServiceProvider.php
touch app/Auth/Sessions.php
touch app/Sessions/2fa.json
touch app/Subscription/SubscriptionCheckout.php app/Subscription/ExpressCheckout.php
touch app/Tenant/Store/Events/SubscriptionActivated.php \
      app/Tenant/Store/Events/SubscriptionCheckout.php \
      app/Tenant/Store/Events/ExpressCheckout.php

# Excluded directories
mkdir -p node_modules/react vendor/illuminate .next/server
touch node_modules/react/package.json node_modules/react/index.js
touch vendor/illuminate/package.json vendor/illuminate/Config.php
touch .next/server/pages.js

# Initial commit
git add -A
git commit -q -m init >/dev/null 2>&1

# Modified files (for git-modified priority + .env.example test)
echo modified > Claude.md
echo modified > .env.example
echo modified > app/Controllers/MediaController.php
mkdir -p admin/components
echo modified > admin/components/createApi.ts
git add -A 2>/dev/null

# ---- Fake transcript so session-awareness ranking works -----------------
SLUG="${PROJ//\//-}"
TRANSCRIPT_DIR="$FAKE_HOME/.claude/projects/$SLUG"
mkdir -p "$TRANSCRIPT_DIR"
cat > "$TRANSCRIPT_DIR/test.jsonl" <<EOF
{"file_path":"$PROJ/routes/api.php"}
{"file_path":"$PROJ/Claude.md"}
{"file_path":"$PROJ/app/Controllers/MediaController.php"}
EOF

# ---- Configure environment for the test runs ----------------------------
export HOME="$FAKE_HOME"
export FSG_WORKSPACE_DIRS="$WS"

PASS=0 FAIL=0

# ---- Run the script under test ------------------------------------------
run() {
  local query="$1" project="$2"
  if grep -q TEST_MODE "$SCRIPT" 2>/dev/null; then
    TEST_MODE=1 CLAUDE_PROJECT_DIR="$project" bash "$SCRIPT" "$query" "$project" 2>/dev/null
  else
    echo "{\"query\":\"$query\"}" | CLAUDE_PROJECT_DIR="$project" bash "$SCRIPT" 2>/dev/null
  fi
}

# ---- Assertion helpers --------------------------------------------------
has() {
  local label="$1" query="$2" project="$3" needle="$4"
  if run "$query" "$project" | grep -qiF "$needle"; then
    printf "  PASS  %s\n" "$label"; PASS=$((PASS+1))
  else
    printf "  FAIL  %s (missing '%s')\n" "$label" "$needle"; FAIL=$((FAIL+1))
  fi
}

first() {
  local label="$1" query="$2" project="$3" needle="$4"
  local f
  f=$(run "$query" "$project" | head -1)
  if echo "$f" | grep -qiF "$needle"; then
    printf "  PASS  %s\n" "$label"; PASS=$((PASS+1))
  else
    printf "  FAIL  %s (first='%s', want '%s')\n" "$label" "$f" "$needle"; FAIL=$((FAIL+1))
  fi
}

count_check() {
  local label="$1" query="$2" project="$3" op="$4" expected="$5"
  local out c
  out=$(run "$query" "$project")
  c=$(printf '%s\n' "$out" | grep -c .)
  [[ -z "$out" ]] && c=0
  local ok=0
  case "$op" in
    ">")  [[ $c -gt $expected ]] && ok=1 ;;
    ">=") [[ $c -ge $expected ]] && ok=1 ;;
    "=")  [[ $c -eq $expected ]] && ok=1 ;;
    "<")  [[ $c -lt $expected ]] && ok=1 ;;
  esac
  if [[ $ok -eq 1 ]]; then
    printf "  PASS  %s (%d results)\n" "$label" "$c"; PASS=$((PASS+1))
  else
    printf "  FAIL  %s (%d results, expected %s %d)\n" "$label" "$c" "$op" "$expected"; FAIL=$((FAIL+1))
  fi
}

excludes() {
  local label="$1" query="$2" project="$3" needle="$4"
  if run "$query" "$project" | grep -qiF "$needle"; then
    printf "  FAIL  %s (should NOT contain '%s')\n" "$label" "$needle"; FAIL=$((FAIL+1))
  else
    printf "  PASS  %s\n" "$label"; PASS=$((PASS+1))
  fi
}

fast() {
  local label="$1" query="$2" project="$3" max="$4"
  local s e ms
  s=$(python3 -c 'import time;print(int(time.time()*1000))')
  run "$query" "$project" > /dev/null
  e=$(python3 -c 'import time;print(int(time.time()*1000))')
  ms=$((e-s))
  if [[ $ms -le $max ]]; then
    printf "  PASS  %s (%dms)\n" "$label" "$ms"; PASS=$((PASS+1))
  else
    printf "  FAIL  %s (%dms > %dms)\n" "$label" "$ms" "$max"; FAIL=$((FAIL+1))
  fi
}

echo "================================================================"
echo "  DX TEST SUITE: $(basename "$SCRIPT")"
echo "  Fixture: $PROJ"
echo "================================================================"

echo ""
echo "── 1. Basic Discovery ──"
count_check "empty browse returns results"          "" "$PROJ" ">" 10
has         "empty browse shows Claude.md"          "" "$PROJ" "Claude.md"
has         "empty browse shows app dir"            "" "$PROJ" "app"
count_check "single char 'a' returns results"       "a" "$PROJ" ">" 5
has         "finds exact filename"                  "artisan" "$PROJ" "artisan"
has         "finds file by extension"               "vitest.config" "$PROJ" "vitest.config"

echo ""
echo "── 2. Deep File Discovery ──"
has   "MediaService at depth 2"          "MediaService" "$PROJ" "MediaService.php"
has   "SubscriptionCheckout depth 2-4"   "SubscriptionCheckout" "$PROJ" "SubscriptionCheckout.php"
has   "ExpressCheckout"                  "ExpressCheckout" "$PROJ" "ExpressCheckout.php"
has   "SubscriptionActivated at depth 4" "SubscriptionActivated" "$PROJ" "SubscriptionActivated.php"
has   "2fa at depth 2"                   "2fa" "$PROJ" "2fa.json"
has   "Sessions at depth 2"              "Sessions" "$PROJ" "Sessions.php"

echo ""
echo "── 3. Typo Resistance ──"
has   "transposed: contorller"          "contorller" "$PROJ" "Controller"
has   "missing letter: chckout"         "chckout" "$PROJ" "checkout"
has   "extra letter: controlller"       "controlller" "$PROJ" "Controller"
has   "scrambled: mdoel"                "mdoel" "$PROJ" "Model"
has   "vowel drop: cnfg"                "cnfg" "$PROJ" "config"
has   "missing middle: mdia"            "mdia" "$PROJ" "media"
has   "keyboard adjacent: contriller"   "contriller" "$PROJ" "Controller"
has   "abbreviated: subs"               "subs" "$PROJ" "Subscription"
has   "case insensitive: CONTROLLER"    "CONTROLLER" "$PROJ" "Controller"
has   "case insensitive: claude.md"     "claude.md" "$PROJ" "Claude.md"

echo ""
echo "── 4. Disambiguation ──"
has   "MediaService vs MediaController"      "MediaServ" "$PROJ" "MediaService"
has   "AuthService vs AuthController"        "AuthServ" "$PROJ" "AuthService"
has   "distinguishes Service from Provider"  "TenantService" "$PROJ" "TenantServiceProvider"
has   "finds exact migration number"         "0007" "$PROJ" "0007_create_contacts"
has   "finds specific test file"             "media.test" "$PROJ" "media.test.ts"
has   "admin vs app PagesController"         "PagesController" "$PROJ" "PagesController"

echo ""
echo "── 5. Git-Modified Priority ──"
first "git-modified ranks first: Claude"        "Claude" "$PROJ" "Claude"
first "git-modified ranks first: controller"    "controller" "$PROJ" "Controller"
has   "shows .env.example (git modified)"       ".env" "$PROJ" ".env.example"
has   "shows createApi (git modified)"          "createApi" "$PROJ" "createApi"

echo ""
echo "── 6. Path Navigation ──"
count_check "browse app/ shows children"            "app/" "$PROJ" ">" 5
count_check "browse admin/ shows children"          "admin/" "$PROJ" ">" 5
count_check "browse config/ shows children"         "config/" "$PROJ" ">" 5
count_check "browse tests/api/ shows tests"         "tests/api/" "$PROJ" ">" 5
has         "path query admin/comp finds files"     "admin/comp" "$PROJ" "components"
has         "deep browse app/Tenant/Store/Events/"  "app/Tenant/Store/Events/" "$PROJ" "Subscription"
has         "browse database/migrations/"           "database/migrations/" "$PROJ" "0001"
count_check "browse nonexistent dir"                "app/nonexistent/" "$PROJ" "=" 0

echo ""
echo "── 7. Rarely-Opened Infrastructure Files ──"
has   "finds BaseConfigurator"        "BaseConfigurator" "$PROJ" "BaseConfigurator.php"
has   "finds TenantBootstrapper"      "TenantBootstrapper" "$PROJ" "TenantBootstrapper.php"
has   "finds SchemaRegistryService"   "SchemaRegistry" "$PROJ" "SchemaRegistryService.php"
has   "finds CapabilityService"       "CapabilityServ" "$PROJ" "CapabilityService.php"
has   "finds BaseJob"                 "BaseJob" "$PROJ" "BaseJob.php"
has   "finds DynamicDataProvider"     "DynamicData" "$PROJ" "DynamicDataProvider.php"

echo ""
echo "── 8. Home Navigation ──"
count_check "~/ returns results"                "~/" "$PROJ" ">" 0
count_check "~/Dev returns results"             "~/Dev" "$PROJ" ">" 0
has         "~/Dev finds Developer"             "~/Dev" "$PROJ" "Developer"
has         "~/Developer/proj expands"          "~/Developer/proj" "$PROJ" "proj"
count_check "~/.claude/ lists config"           "~/.claude/" "$PROJ" ">" 5
has         "~/.claude/ has history"            "~/.claude/" "$PROJ" "history"

echo ""
echo "── 8b. Workspace Cross-Project Discovery ──"
has         "cross-project: sibling"            "sibling" "$PROJ" "sibling"
has         "cross-project: agentation"         "agentation" "$PROJ" "agentation"

echo ""
echo "── 9. Edge Cases ──"
has         "dot prefix .env"                   ".env" "$PROJ" ".env"
has         "dot prefix .gitignore"             ".gitignore" "$PROJ" ".gitignore"
has         "dot prefix .editorconfig"          ".editorconfig" "$PROJ" ".editorconfig"
count_check "nonexistent returns 0"             "xyznonexistent999" "$PROJ" "=" 0
count_check "empty query sibling project"       "" "$SIBLING" ">" 0
has         "sibling has README"                "README" "$SIBLING" "README"

echo ""
echo "── 10. Claude Code Session Awareness ──"
has   "transcript surfaces api.php"     "api" "$PROJ" "api.php"
has   "transcript surfaces Claude.md"   "Claude.md" "$PROJ" "Claude.md"

echo ""
echo "── 11. Result Quality ──"
excludes  "no node_modules in results"        "react" "$PROJ" "node_modules"
excludes  "no vendor in results"              "illuminate" "$PROJ" "vendor"
excludes  "no .git in results"                ".git" "$PROJ" ".git/objects"
count_check "max 15 results"                  "a" "$PROJ" "<" 16

echo ""
echo "── 12. Workspace Path Navigation ──"
count_check "references/ lists workspace dir"       "references/" "$PROJ" ">" 1
has         "references/ shows mago"                "references/" "$PROJ" "mago"
has         "references/ shows agentation"          "references/" "$PROJ" "agentation"
has         "references/mago finds mago"            "references/mago" "$PROJ" "mago"
has         "references/agent fuzzy matches"        "references/agent" "$PROJ" "agent"
count_check "references/mago/ lists contents"       "references/mago/" "$PROJ" ">" 0
has         "references/mago/ shows src"            "references/mago/" "$PROJ" "src"
has         "references/mago/ shows crates"         "references/mago/" "$PROJ" "crates"
has         "refernces/ typo in first segment"      "refernces/" "$PROJ" "mago"
has         "references/excaldraw typo in second"   "references/excaldraw" "$PROJ" "excalidraw"
has         "sibling/ cross-workspace browse"       "sibling/" "$PROJ" "src"
has         "sibling/src finds src dir"             "sibling/src" "$PROJ" "src"
count_check "app/ local nav unaffected"             "app/" "$PROJ" ">" 5
has         "app/Tenant local nav unaffected"       "app/Tenant" "$PROJ" "Tenant"
count_check "config/ local nav unaffected"          "config/" "$PROJ" ">" 5

echo ""
echo "── 13. Prefix Scope — Local Directory Prefixes (Scope A) ──"
has         "local prefix scope: .claude/md finds Claude.md"      ".claude/md" "$PROJ" ".claude/Claude.md"
first       "local prefix leads: .claude/ → inside .claude/"      ".claude/" "$PROJ" ".claude/"
has         "recursive: .claude/md finds nested agents/"          ".claude/md" "$PROJ" "agents/frontend"
count_check ".claude/md returns ≥3 results"                       ".claude/md" "$PROJ" ">=" 3
has         "filter in prefix: database/migrations/ finds 0001"   "database/migrations/" "$PROJ" "0001"
has         "filter in prefix: admin/comp finds components"       "admin/comp" "$PROJ" "components"
has         "includes directories: .claude/ shows subdirs"        ".claude/" "$PROJ" "agents"
count_check "nonexistent local prefix: app/zzznope/"              "app/zzznope/" "$PROJ" "=" 0
has         "workspace prefix unaffected: references/mago"        "references/mago" "$PROJ" "mago"
has         "home prefix unaffected: ~/Dev"                       "~/Dev" "$PROJ" "Developer"
count_check "cap enforced with prefix + fallback: app/"           "app/" "$PROJ" "<" 16

echo ""
echo "── 14. Performance (< 200ms) ──"
fast "empty browse"             "" "$PROJ" 200
fast "controller"               "controller" "$PROJ" 200
fast "single char a"            "a" "$PROJ" 200
fast "deep: ExpressCheckout"    "ExpressCheckout" "$PROJ" 200
fast "typo: contorller"         "contorller" "$PROJ" 200
fast "global: ~/Dev"            "~/Dev" "$PROJ" 200
fast "path: app/Ten"            "app/Ten" "$PROJ" 200
fast "Claude.md"                "Claude.md" "$PROJ" 200
fast "browse: app/"             "app/" "$PROJ" 200
fast "ws browse: references/"   "references/" "$PROJ" 200
fast "ws deep: references/mago" "references/mago" "$PROJ" 200
fast "empty browse sibling"     "" "$SIBLING" 200
fast "prefix scope: .claude/md" ".claude/md" "$PROJ" 200

echo ""
echo "================================================================"
printf "  TOTAL: %d passed, %d failed out of %d\n" "$PASS" "$FAIL" "$((PASS+FAIL))"
echo "================================================================"
exit $FAIL

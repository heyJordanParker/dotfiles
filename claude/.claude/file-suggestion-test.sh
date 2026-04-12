#!/bin/bash
# DX test suite for file-suggestion.sh
# 79 scenarios covering discovery, typos, git priority, globals, edge cases, and speed
#
# Usage:
#   ./file-suggestion-test.sh                    # test the deployed script
#   ./file-suggestion-test.sh /path/to/script.sh # test a specific script
#
# Requirements:
#   - ~/Developer/creator-income-blueprint must exist (primary test project)
#   - ~/dotfiles must exist
#   - The script under test must support TEST_MODE=1 (query as $1, project dir as $2)
#     OR accept {"query":"..."} on stdin with CLAUDE_PROJECT_DIR env var

SCRIPT="${1:-$(dirname "$0")/file-suggestion.sh}"
[[ ! -f "$SCRIPT" ]] && { echo "Script not found: $SCRIPT"; exit 1; }

CIB="$HOME/Developer/creator-income-blueprint"
DOT="$HOME/dotfiles"
PASS=0 FAIL=0

[[ ! -d "$CIB" ]] && { echo "Test project not found: $CIB"; exit 1; }
[[ ! -d "$DOT" ]] && { echo "Dotfiles not found: $DOT"; exit 1; }

# Run the script under test
run() {
  local query="$1" project="$2"
  if grep -q 'TEST_MODE' "$SCRIPT" 2>/dev/null; then
    TEST_MODE=1 CLAUDE_PROJECT_DIR="$project" bash "$SCRIPT" "$query" "$project" 2>/dev/null
  else
    echo "{\"query\":\"$query\"}" | CLAUDE_PROJECT_DIR="$project" bash "$SCRIPT" 2>/dev/null
  fi
}

# Assertions
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
  local f=$(run "$query" "$project" | head -1)
  if echo "$f" | grep -qiF "$needle"; then
    printf "  PASS  %s\n" "$label"; PASS=$((PASS+1))
  else
    printf "  FAIL  %s (first='%s', want '%s')\n" "$label" "$f" "$needle"; FAIL=$((FAIL+1))
  fi
}

count_check() {
  local label="$1" query="$2" project="$3" op="$4" expected="$5"
  local c=$(run "$query" "$project" | grep -c . 2>/dev/null)
  [[ -z "$(run "$query" "$project")" ]] && c=0
  local ok=0
  case "$op" in
    ">") [[ $c -gt $expected ]] && ok=1 ;;
    ">=") [[ $c -ge $expected ]] && ok=1 ;;
    "=") [[ $c -eq $expected ]] && ok=1 ;;
    "<") [[ $c -lt $expected ]] && ok=1 ;;
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
  local s=$(python3 -c 'import time;print(int(time.time()*1000))')
  run "$query" "$project" > /dev/null
  local e=$(python3 -c 'import time;print(int(time.time()*1000))')
  local ms=$((e-s))
  if [[ $ms -le $max ]]; then
    printf "  PASS  %s (%dms)\n" "$label" "$ms"; PASS=$((PASS+1))
  else
    printf "  FAIL  %s (%dms > %dms)\n" "$label" "$ms" "$max"; FAIL=$((FAIL+1))
  fi
}

echo "================================================================"
echo "  DX TEST SUITE: $(basename "$SCRIPT")"
echo "================================================================"

echo ""
echo "── 1. Basic Discovery ──"
count_check "empty browse returns results"          "" "$CIB" ">" 10
has         "empty browse shows Claude.md"          "" "$CIB" "Claude.md"
has         "empty browse shows app dir"            "" "$CIB" "app"
count_check "single char 'a' returns results"       "a" "$CIB" ">" 5
has         "finds exact filename"                  "artisan" "$CIB" "artisan"
has         "finds file by extension"               "vitest.config" "$CIB" "vitest.config"

echo ""
echo "── 2. Deep File Discovery ──"
has   "depth 4: MediaService"           "MediaService" "$CIB" "MediaService.php"
has   "depth 5: SubscriptionCheckout"   "SubscriptionCheckout" "$CIB" "SubscriptionCheckout.php"
has   "depth 6: ExpressCheckout"        "ExpressCheckout" "$CIB" "ExpressCheckout.php"
has   "depth 6: SubscriptionActivated"  "SubscriptionActivated" "$CIB" "SubscriptionActivated.php"
has   "depth 7: 2fa.json"              "2fa" "$CIB" "2fa.json"
has   "depth 6: Sessions.php"          "Sessions" "$CIB" "Sessions.php"

echo ""
echo "── 3. Typo Resistance ──"
has   "transposed: contorller"          "contorller" "$CIB" "Controller"
has   "missing letter: chckout"         "chckout" "$CIB" "checkout"
has   "extra letter: controlller"       "controlller" "$CIB" "Controller"
has   "scrambled: mdoel"                "mdoel" "$CIB" "Model"
has   "vowel drop: cnfg"               "cnfg" "$CIB" "config"
has   "missing middle: mdia"            "mdia" "$CIB" "media"
has   "keyboard adjacent: contriller"   "contriller" "$CIB" "Controller"
has   "abbreviated: subs"              "subs" "$CIB" "Subscription"
has   "case insensitive: CONTROLLER"    "CONTROLLER" "$CIB" "Controller"
has   "case insensitive: claude.md"     "claude.md" "$CIB" "Claude.md"

echo ""
echo "── 4. Disambiguation ──"
has   "MediaService vs MediaController"      "MediaServ" "$CIB" "MediaService"
has   "AuthService vs AuthController"        "AuthServ" "$CIB" "AuthService"
has   "distinguishes Service from Provider"  "TenantService" "$CIB" "TenantServiceProvider"
has   "finds exact migration number"         "0007" "$CIB" "0007_create_contacts"
has   "finds specific test file"             "media.test" "$CIB" "media.test.ts"
has   "admin vs app Controller"              "PagesController" "$CIB" "PagesController"

echo ""
echo "── 5. Git-Modified Priority ──"
first "git-modified ranks first: Claude"    "Claude" "$CIB" "Claude"
first "git-modified ranks first: controller" "controller" "$CIB" "Controller"
has   "shows .env.example (git modified)"   ".env" "$CIB" ".env.example"
has   "shows createApi (git modified)"      "createApi" "$CIB" "createApi"

echo ""
echo "── 6. Path Navigation ──"
count_check "browse app/ shows children"            "app/" "$CIB" ">" 5
count_check "browse admin/ shows children"          "admin/" "$CIB" ">" 5
count_check "browse config/ shows children"         "config/" "$CIB" ">" 5
count_check "browse tests/api/ shows tests"         "tests/api/" "$CIB" ">" 5
has         "path query admin/comp finds files"     "admin/comp" "$CIB" "components"
has         "deep browse app/Tenant/Store/Events/"  "app/Tenant/Store/Events/" "$CIB" "Subscription"
has         "browse database/migrations/"           "database/migrations/" "$CIB" "0001"
count_check "browse nonexistent dir"                "app/nonexistent/" "$CIB" "=" 0

echo ""
echo "── 7. Rarely-Opened Infrastructure Files ──"
has   "finds BaseConfigurator"        "BaseConfigurator" "$CIB" "BaseConfigurator.php"
has   "finds TenantBootstrapper"      "TenantBootstrapper" "$CIB" "TenantBootstrapper.php"
has   "finds SchemaRegistryService"   "SchemaRegistry" "$CIB" "SchemaRegistryService.php"
has   "finds CapabilityService"       "CapabilityServ" "$CIB" "CapabilityService.php"
has   "finds BaseJob"                 "BaseJob" "$CIB" "BaseJob.php"
has   "finds DynamicDataProvider"     "DynamicData" "$CIB" "DynamicDataProvider.php"

echo ""
echo "── 8. Home Navigation ──"
count_check "~/ returns results"               "~/" "$CIB" ">" 5
count_check "~/Dev returns results"             "~/Dev" "$CIB" ">" 3
has         "~/Dev finds Developer"             "~/Dev" "$CIB" "Developer"
has         "~/Developer/creator expands"       "~/Developer/creator" "$CIB" "creator-income-blueprint"
count_check "~/.claude/ lists config"           "~/.claude/" "$CIB" ">" 10
has         "~/.claude/ has history"            "~/.claude/" "$CIB" "history"

echo ""
echo "── 8b. Workspace Discovery ──"
has         "cross-project: agentect"           "agentect" "$CIB" "agentect"
has         "cross-project: drawbridge"         "drawbridge" "$CIB" "drawbridge"

echo ""
echo "── 9. Edge Cases ──"
has         "dot prefix .env"                   ".env" "$CIB" ".env"
has         "dot prefix .gitignore"             ".gitignore" "$CIB" ".gitignore"
has         "dot prefix .editorconfig"          ".editorconfig" "$CIB" ".editorconfig"
count_check "nonexistent returns 0"             "xyznonexistent999" "$CIB" "=" 0
count_check "empty query dotfiles project"      "" "$DOT" ">" 10
has         "dotfiles finds ghostty"            "ghostty" "$DOT" "ghostty"
has         "dotfiles finds karabiner"          "karabiner" "$DOT" "karabiner"

echo ""
echo "── 10. Claude Code Session Awareness ──"
has   "transcript files surface for route"  "route" "$CIB" "route"
has   "finds recently read files"           "Claude.md" "$CIB" "Claude.md"

echo ""
echo "── 11. Result Quality ──"
excludes  "no node_modules in results"        "react" "$CIB" "node_modules"
excludes  "no vendor in results"              "illuminate" "$CIB" "vendor"
excludes  "no .git in results"                ".git" "$CIB" ".git/objects"
count_check "max 15 results"                  "a" "$CIB" "<" 16

echo ""
echo "── 12. Workspace Path Navigation ──"
count_check "references/ lists workspace dir"       "references/" "$CIB" ">" 3
has         "references/ shows mago"                "references/" "$CIB" "mago"
has         "references/ shows agentation"          "references/" "$CIB" "agentation"
has         "references/mago finds mago"            "references/mago" "$CIB" "mago"
has         "references/agent fuzzy matches"        "references/agent" "$CIB" "agent"
count_check "references/mago/ lists contents"       "references/mago/" "$CIB" ">" 3
has         "references/mago/ shows src"            "references/mago/" "$CIB" "src"
has         "references/mago/ shows crates"         "references/mago/" "$CIB" "crates"
has         "refernces/ typo in first segment"      "refernces/" "$CIB" "mago"
has         "references/excaldraw typo in second"   "references/excaldraw" "$CIB" "excalidraw"
has         "dotfiles/ cross-workspace browse"      "dotfiles/" "$CIB" "claude"
has         "dotfiles/claude finds claude dir"      "dotfiles/claude" "$CIB" "claude"
count_check "app/ local nav unaffected"             "app/" "$CIB" ">" 5
has         "app/Tenant local nav unaffected"       "app/Tenant" "$CIB" "Tenant"
count_check "config/ local nav unaffected"          "config/" "$CIB" ">" 5

echo ""
echo "── 13. Performance (< 90ms) ──"
fast "empty browse"          "" "$CIB" 90
fast "controller"            "controller" "$CIB" 90
fast "single char a"         "a" "$CIB" 90
fast "deep: ExpressCheckout" "ExpressCheckout" "$CIB" 90
fast "typo: contorller"      "contorller" "$CIB" 90
fast "global: ~/Dev"         "~/Dev" "$CIB" 90
fast "path: app/Ten"         "app/Ten" "$CIB" 90
fast "Claude.md"             "Claude.md" "$CIB" 90
fast "browse: app/"          "app/" "$CIB" 90
fast "ws browse: references/" "references/" "$CIB" 90
fast "ws deep: references/mago" "references/mago" "$CIB" 90
fast "empty browse dotfiles" "" "$DOT" 90

echo ""
echo "================================================================"
printf "  TOTAL: %d passed, %d failed out of %d\n" "$PASS" "$FAIL" "$((PASS+FAIL))"
echo "================================================================"
exit $FAIL

# React Security Best Practices

## Understand JSX Auto-Escaping Limits

React escapes content in JSX `{value}` bindings, converting `<`, `>`, `&`, `"`, `'` to HTML entities. This prevents HTML injection in text content but does NOT protect against `dangerouslySetInnerHTML`, `javascript:` URLs in href/src, direct DOM manipulation via refs, or SSR string concatenation.

```javascript
// Incorrect: React does NOT validate URL protocols
<a href={userProvidedUrl}>Click</a>

// Incorrect: Ref manipulation bypasses React entirely
ref.current.innerHTML = userControlledValue;

// Correct: JSX text binding is auto-escaped
<p>{userInput}</p>
```

## Sanitize dangerouslySetInnerHTML with DOMPurify

Never pass unsanitized HTML to `dangerouslySetInnerHTML`. Encapsulate usage in a `SafeHTML` wrapper so linters can flag raw usage elsewhere.

```javascript
// Incorrect: Raw HTML from CMS or user input
<div dangerouslySetInnerHTML={{ __html: htmlFromApi }} />

// Correct: Always sanitize, encapsulate in a wrapper component
import DOMPurify from 'dompurify';

function SafeHTML({ html, allowedTags, allowedAttributes }) {
  const clean = DOMPurify.sanitize(html, {
    ALLOWED_TAGS: allowedTags,
    ALLOWED_ATTR: allowedAttributes,
  });
  return <div dangerouslySetInnerHTML={{ __html: clean }} />;
}

<SafeHTML html={htmlFromApi} />
```

## Validate User-Provided URLs with Protocol Allowlist

React warns about `javascript:` URLs in development but does not block them. Always validate URL protocols against an allowlist. Prefer accepting identifiers over full URLs.

```javascript
// Incorrect: Unvalidated user URL enables script injection
<a href={userUrl}>Profile</a>

// Correct: Allowlist safe protocols
function validateURL(url) {
  try {
    const parsed = new URL(url);
    return ['https:', 'http:', 'mailto:'].includes(parsed.protocol);
  } catch {
    return false;
  }
}

<a href={validateURL(userUrl) ? userUrl : '#'}>Profile</a>
```

## Store Auth Tokens Securely

Access tokens in localStorage are stolen by any XSS. Store access tokens in memory (React state) and refresh tokens in httpOnly cookies.

```javascript
// Incorrect: Any injected script can steal this
localStorage.setItem('token', jwt);

// Correct: Server sets httpOnly cookie for refresh token
res.cookie('refresh_token', refreshToken, {
  httpOnly: true,
  secure: true,
  sameSite: 'Strict',
  path: '/api/refresh',
  maxAge: 7 * 24 * 60 * 60 * 1000,
});
// Client stores access token in memory only
const [accessToken, setAccessToken] = useState(null);
```

## Never Execute User-Controlled Strings

`eval()`, `new Function()`, and string-form `setTimeout()` execute arbitrary code. There is no safe way to use them with untrusted input.

```javascript
// Incorrect: All of these execute attacker-controlled code
eval(userInput);
new Function(userInput)();
setTimeout(userInput, 1000);

// Correct: Parse data, never execute it
const config = JSON.parse(userInput);
```

## Use Nonce-Based CSP in Production

Content Security Policy prevents inline script injection. Generate a unique nonce per request. Never use `unsafe-inline` or `unsafe-eval` for scripts in production.

```javascript
// Correct: Generate nonce per request in middleware
const nonce = crypto.randomBytes(16).toString('base64');
const csp = [
  `default-src 'self'`,
  `script-src 'self' 'nonce-${nonce}'`,
  `style-src 'self' 'nonce-${nonce}'`,
  `frame-ancestors 'none'`,
].join('; ');
response.headers.set('Content-Security-Policy', csp);
```

## Pin Dependencies and Audit Regularly

Pin exact versions, commit lockfiles, use `npm ci` (not `npm install`) in CI. Run `npm audit` regularly.

```bash
npm install --save-exact some-package
git add package-lock.json
npm ci    # CI: fails if lockfile doesn't match package.json
npm audit
```

## Validate Redirects and Prevent SSRF

Unvalidated redirects enable phishing. User-controlled URLs in server-side fetches enable SSRF.

```javascript
// Incorrect: Open redirect
navigate(searchParams.get('returnUrl'));

// Correct: Same-origin redirects only
const url = new URL(searchParams.get('returnUrl'), window.location.origin);
if (url.origin !== window.location.origin) { navigate('/dashboard'); return; }
navigate(url.pathname);

// Incorrect: SSRF via user-controlled fetch URL
const data = await fetch(searchParams.get('apiUrl'));

// Correct: Allowlist of known endpoints
const APIS = { users: 'https://api.example.com/users' };
const endpoint = APIS[searchParams.get('resource')];
if (!endpoint) throw new Error('Invalid resource');
const data = await fetch(endpoint);
```

## Keep React Patched

Critical RCE vulnerabilities have been disclosed in React 19.x (CVE-2025-55182, CVSS 10.0). Update immediately when security patches are released. Pin exact versions and monitor advisories.

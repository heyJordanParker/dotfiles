# Security

One Process: keep User input untrusted, put unsafe behavior behind narrow wrappers, and verify server and client boundaries.

## 1. Treat JSX escaping as narrow protection

### Understand JSX auto-escaping limits

React escapes content in JSX `{value}` bindings by converting `<`, `>`, `&`, `"`, and `'` to HTML entities. This prevents HTML injection in text content but does not protect `dangerouslySetInnerHTML`, `javascript:` URLs in `href` or `src`, direct DOM manipulation through refs, or server-side rendering string concatenation.

Never:
  ```javascript
  <a href={userProvidedUrl}>Click</a>
  ref.current.innerHTML = userControlledValue;
  ```

Example:
  ```javascript
  <p>{userInput}</p>
  ```

## 2. Encapsulate unsafe HTML

### Sanitize dangerouslySetInnerHTML with DOMPurify

Never pass unsanitized HTML to `dangerouslySetInnerHTML`. Encapsulate usage in a `SafeHTML` wrapper so linters can flag raw usage elsewhere.

Never:
  ```javascript
  <div dangerouslySetInnerHTML={{ __html: htmlFromApi }} />
  ```

Example:
  ```javascript
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

## 3. Validate URLs before rendering or fetching

### Validate User-provided URLs with a protocol allowlist

React warns about `javascript:` URLs in development but does not block them. Validate URL protocols against an allowlist. Prefer accepting identifiers over full URLs.

Never:
  ```javascript
  <a href={userUrl}>Profile</a>
  ```

Example:
  ```javascript
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

### Validate redirects and prevent server-side request forgery

Unvalidated redirects enable phishing. User-controlled URLs in server-side fetches enable server-side request forgery.

Never:
  ```javascript
  navigate(searchParams.get('returnUrl'));
  const data = await fetch(searchParams.get('apiUrl'));
  ```

Example:
  ```javascript
  const url = new URL(searchParams.get('returnUrl'), window.location.origin);
  if (url.origin !== window.location.origin) { navigate('/dashboard'); return; }
  navigate(url.pathname);

  const APIS = { users: 'https://api.example.com/users' };
  const endpoint = APIS[searchParams.get('resource')];
  if (!endpoint) throw new Error('Invalid resource');
  const data = await fetch(endpoint);
  ```

## 4. Store authentication tokens safely

### Store access tokens in memory and refresh tokens in httpOnly cookies

Access tokens in localStorage are stolen by any cross-site scripting issue.

Never:
  ```javascript
  localStorage.setItem('token', jwt);
  ```

Example:
  ```javascript
  res.cookie('refresh_token', refreshToken, {
    httpOnly: true,
    secure: true,
    sameSite: 'Strict',
    path: '/api/refresh',
    maxAge: 7 * 24 * 60 * 60 * 1000,
  });

  const [accessToken, setAccessToken] = useState(null);
  ```

## 5. Do not execute User-controlled strings

### Parse data, never execute it

`eval()`, `new Function()`, and string-form `setTimeout()` execute arbitrary code. There is no safe way to use them with untrusted input.

Never:
  ```javascript
  eval(userInput);
  new Function(userInput)();
  setTimeout(userInput, 1000);
  ```

Example:
  ```javascript
  const config = JSON.parse(userInput);
  ```

## 6. Lock down script execution

### Use nonce-based Content Security Policy in production

Content Security Policy prevents inline script injection. Generate a unique nonce per request. Never use `unsafe-inline` or `unsafe-eval` for scripts in production.

Example:
  ```javascript
  const nonce = crypto.randomBytes(16).toString('base64');
  const csp = [
    `default-src 'self'`,
    `script-src 'self' 'nonce-${nonce}'`,
    `style-src 'self' 'nonce-${nonce}'`,
    `frame-ancestors 'none'`,
  ].join('; ');
  response.headers.set('Content-Security-Policy', csp);
  ```

## 7. Keep dependencies patched and reproducible

### Pin dependencies and audit regularly

Pin exact versions, commit lockfiles, use `npm ci` instead of `npm install` in continuous integration, and run `npm audit` regularly.

Example:
  ```bash
  npm install --save-exact some-package
  git add package-lock.json
  npm ci
  npm audit
  ```

### Keep React patched

Critical remote code execution vulnerabilities have been disclosed in React 19.x: CVE-2025-55182 with CVSS 10.0. Update immediately when security patches are released. Pin exact versions and monitor advisories.

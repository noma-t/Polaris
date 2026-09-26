import { useEffect, useState, type FormEvent } from "react";
import { OverlayScrollArea } from "../components/OverlayScrollArea";
import { PolarisLogo } from "../components/PolarisLogo";
import { EyeIcon } from "../components/icons";
import { describeAuthError, login, verifyTwoFactor, type CurrentUser, type TwoFactorMethod } from "../lib/auth";

const CODE_LENGTH = 6;

const TWO_FACTOR_HINTS: Record<TwoFactorMethod, string> = {
  totp: "Enter the 6-digit code from your authenticator app.",
  emailOtp: "Enter the 6-digit code sent to your email.",
};

interface LoginScreenProps {
  onSignedIn: (user: CurrentUser) => void;
  onError: (message: string) => void;
}

export function LoginScreen({ onSignedIn, onError }: LoginScreenProps) {
  const [step, setStep] = useState<1 | 2>(1);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [code, setCode] = useState("");
  const [twoFactorMethod, setTwoFactorMethod] = useState<TwoFactorMethod>("totp");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const isCodeComplete = code.length === CODE_LENGTH;
  const canSubmitCredentials = username.trim() !== "" && password !== "" && !isSubmitting;

  const appendDigit = (d: string) => setCode((c) => (c + d).slice(0, CODE_LENGTH));
  const deleteDigit = () => setCode((c) => c.slice(0, -1));

  const submitCredentials = async (e: FormEvent) => {
    e.preventDefault();
    if (!canSubmitCredentials) return;
    setIsSubmitting(true);
    try {
      const result = await login(username.trim(), password);
      if (result.kind === "signedIn") {
        onSignedIn(result.user);
        return;
      }
      // 複数提示された場合は authenticator app (TOTP) を優先する
      setTwoFactorMethod(result.methods.includes("totp") ? "totp" : "emailOtp");
      setCode("");
      setStep(2);
    } catch (error) {
      onError(describeAuthError(error));
    } finally {
      setIsSubmitting(false);
    }
  };

  const verify = async () => {
    if (!isCodeComplete || isSubmitting) return;
    setIsSubmitting(true);
    try {
      onSignedIn(await verifyTwoFactor(twoFactorMethod, code));
    } catch (error) {
      onError(describeAuthError(error));
      setCode("");
      setIsSubmitting(false);
    }
  };


  // Step 2 では物理キーボードの数字・Backspace・Enter も受け付ける
  useEffect(() => {
    if (step !== 2 || isSubmitting) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (/^[0-9]$/.test(e.key)) appendDigit(e.key);
      else if (e.key === "Backspace") deleteDigit();
      else if (e.key === "Enter") verify();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  const keypadKeys = [
    ..."123456789".split("").map((d) => ({ label: d, isDigit: true, onPress: () => appendDigit(d) })),
    { label: "Clear", isDigit: false, onPress: () => setCode("") },
    { label: "0", isDigit: true, onPress: () => appendDigit("0") },
    { label: "Delete", isDigit: false, onPress: deleteDigit },
  ];

  return (
    <OverlayScrollArea viewportClassName="login-screen">
      <div className="login-card">
        <div className="login-heading">
          <div className="login-brand">
            <PolarisLogo className="login-brand-logo" />
            <span className="login-brand-name">Polaris</span>
          </div>
          <h1 className="login-title">{step === 2 ? "Verification code" : "Sign in"}</h1>
          {step === 2 && <p className="login-2fa-hint">{TWO_FACTOR_HINTS[twoFactorMethod]}</p>}
        </div>

        {step === 1 && (
          <form className="login-form" onSubmit={submitCredentials}>
            <label className="login-field">
              <span className="login-field-label">Username or email</span>
              <input
                className="text-input login-username-input"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                autoComplete="username"
                autoFocus
              />
            </label>
            <div className="login-field">
              <label className="login-field-label" htmlFor="login-password">
                Password
              </label>
              <div className="login-password-row">
                <input
                  id="login-password"
                  className="text-input login-password-input"
                  type={showPassword ? "text" : "password"}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  autoComplete="current-password"
                />
                <button
                  type="button"
                  className="login-password-toggle"
                  onClick={() => setShowPassword((v) => !v)}
                  aria-label={showPassword ? "Hide password" : "Show password"}
                >
                  <EyeIcon size={22} crossed={showPassword} />
                </button>
              </div>
            </div>
            <button type="submit" className="btn-accent login-submit-button" disabled={!canSubmitCredentials}>
              {isSubmitting ? "Signing in…" : "Next"}
            </button>
          </form>
        )}

        {step === 2 && (
          <div className="login-2fa">
            <div className="code-cells" aria-label="Verification code" aria-live="polite">
              {Array.from({ length: CODE_LENGTH }, (_, i) => (
                <div key={i} className={`code-cell ${i === code.length ? "is-current" : ""}`}>
                  {code[i] ?? ""}
                </div>
              ))}
            </div>
            <div className="keypad">
              {keypadKeys.map((k) => (
                <button
                  key={k.label}
                  className={`keypad-key ${k.isDigit ? "is-digit" : "is-action"}`}
                  onClick={k.onPress}
                  disabled={isSubmitting}
                >
                  {k.label}
                </button>
              ))}
            </div>
            <div className="login-2fa-actions">
              <button className="btn-outline login-back-button" onClick={() => setStep(1)} disabled={isSubmitting}>
                Back
              </button>
              <button className="btn-accent login-verify-button" onClick={verify} disabled={!isCodeComplete || isSubmitting}>
                {isSubmitting ? "Verifying…" : "Sign in"}
              </button>
            </div>
          </div>
        )}

        <div className="login-disclaimer">
          <div>このツールはVRChatとの関係は一切ありません。</div>
          <div>ログイン情報はVRChatにのみ送信されます。</div>
        </div>
      </div>
    </OverlayScrollArea>
  );
}

import { useEffect, useState, type InputHTMLAttributes } from "react";
import { Input } from "./input";

type NativeProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "type" | "value" | "onChange" | "min" | "max"
>;

export function NumericInput({
  value,
  onValueChange,
  min,
  max,
  onBlur,
  ...props
}: NativeProps & {
  value: number;
  onValueChange: (value: number) => void;
  min: number;
  max: number;
}) {
  const [draft, setDraft] = useState(String(value));

  useEffect(() => setDraft(String(value)), [value]);

  return (
    <Input
      {...props}
      type="text"
      inputMode="numeric"
      autoComplete="off"
      spellCheck={false}
      value={draft}
      onChange={(e) => {
        const digits = e.target.value.replace(/\D/g, "").slice(0, String(max).length);
        setDraft(digits);
        const next = Number(digits);
        if (digits && next >= min && next <= max) onValueChange(next);
      }}
      onBlur={(e) => {
        setDraft(String(value));
        onBlur?.(e);
      }}
    />
  );
}

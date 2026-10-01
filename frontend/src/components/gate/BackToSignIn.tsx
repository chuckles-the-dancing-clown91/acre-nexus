import Link from "next/link";
import { ArrowLeft } from "lucide-react";

export function BackToSignIn() {
  return (
    <Link
      href="/login"
      className="mt-6 inline-flex items-center gap-1.5 text-[13px] font-medium text-fg-3 transition hover:text-fg"
    >
      <ArrowLeft className="size-3.5" />
      Back to sign in
    </Link>
  );
}

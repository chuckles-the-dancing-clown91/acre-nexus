import { redirect } from "next/navigation";

export default function AccountHome() {
  redirect("/account/maintenance");
}

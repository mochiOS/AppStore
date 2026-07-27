import Link from "next/link";

export default function NotFound() {
  return <div className="not-found"><h1>見つかりません</h1><p>指定されたアプリは公開されていません。</p><Link href="/">App Storeへ戻る</Link></div>;
}

"use client";

export default function StoreError({ reset }: { error: Error & { digest?: string }; reset: () => void }) {
  return (
    <div className="not-found" role="alert">
      <h1>App Storeを読み込めませんでした</h1>
      <p>しばらくしてから、もう一度お試しください。</p>
      <button className="retry-button" type="button" onClick={() => reset()}>再読み込み</button>
    </div>
  );
}

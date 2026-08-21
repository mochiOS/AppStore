export default function MaintenancePage() {
  return (
    <main className="maintenance-page">
      <section className="maintenance-card" aria-labelledby="maintenance-title">
        <p className="maintenance-brand">mochiOS App Store</p>
        <h1 id="maintenance-title">現在メンテナンス中です</h1>
        <p className="maintenance-message">
          App Storeは公開に向けて準備を進めています。
          <br />
          ご利用いただけるようになるまで、しばらくお待ちください。
        </p>
      </section>
    </main>
  );
}

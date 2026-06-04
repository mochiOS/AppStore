export default function SettingsPage() {
  return (
    <div className="space-y-6">
      <section className="rounded-[2rem] border border-zinc-200 bg-white p-6">
        <p className="text-xs font-medium uppercase tracking-[0.32em] text-zinc-500">Settings</p>
        <h3 className="mt-3 text-2xl font-semibold tracking-tight text-zinc-950">Settings</h3>
      </section>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        <div className="rounded-[2rem] border border-zinc-200 bg-white p-6">
          <h4 className="text-lg font-semibold tracking-tight text-zinc-950">Team</h4>
          <p className="mt-2 text-sm text-zinc-600">3 members</p>
        </div>

        <div className="rounded-[2rem] border border-zinc-200 bg-white p-6">
          <h4 className="text-lg font-semibold tracking-tight text-zinc-950">API access</h4>
          <p className="mt-2 text-sm text-zinc-600">Keys and scopes</p>
        </div>

        <div className="rounded-[2rem] border border-zinc-200 bg-white p-6">
          <h4 className="text-lg font-semibold tracking-tight text-zinc-950">Notifications</h4>
          <p className="mt-2 text-sm text-zinc-600">Enabled</p>
        </div>
      </section>
    </div>
  );
}

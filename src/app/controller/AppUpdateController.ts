import { check, Update } from '@tauri-apps/plugin-updater'

export interface AppUpdateState {
    visible: boolean
    phase: 'idle' | 'checking' | 'available' | 'current' | 'downloading' | 'installing' | 'failed'
    version?: string
    notes?: string
    error?: string
    downloaded: number
    total?: number
}

export class AppUpdateController {
    state: AppUpdateState = { visible: false, phase: 'idle', downloaded: 0 }
    private update: Update | null = null
    private listeners = new Set<() => void>()

    constructor(private checkUpdate = check) {}

    subscribe = (listener: () => void) => {
        this.listeners.add(listener)
        return () => { this.listeners.delete(listener) }
    }

    private set(update: Partial<AppUpdateState>) {
        this.state = { ...this.state, ...update }
        this.listeners.forEach(listener => listener())
    }

    async check(silent = false) {
        if (!silent) this.set({ visible: true })
        if (['checking', 'downloading', 'installing', 'available'].includes(this.state.phase)) return
        this.set({ phase: 'checking', error: undefined, version: undefined, notes: undefined, downloaded: 0, total: undefined })
        try {
            this.update = await this.checkUpdate({ timeout: 20_000 })
            if (this.update) {
                this.set({ visible: true, phase: 'available', version: this.update.version, notes: this.update.body })
            } else this.set({ phase: 'current' })
        } catch (error) {
            // Startup checks stay quiet when offline. Manual checks show a retryable error.
            this.set({ phase: 'failed', error: String(error) })
        }
    }

    dismiss = () => {
        if (['downloading', 'installing'].includes(this.state.phase)) return
        if (this.update) {
            void this.update.close().catch(() => {})
            this.update = null
            this.set({ phase: 'idle' })
        }
        this.set({ visible: false })
    }

    async install() {
        if (!this.update || this.state.phase !== 'available') return
        const update = this.update
        this.set({ phase: 'downloading', downloaded: 0, total: undefined, error: undefined })
        try {
            let downloaded = 0
            await update.download(event => {
                if (event.event === 'Started') this.set({ total: event.data.contentLength })
                if (event.event === 'Progress') {
                    downloaded += event.data.chunkLength
                    this.set({ downloaded })
                }
            }, { timeout: 30 * 60_000 })
            // The native plugin verifies the signature before launching the installer.
            this.set({ phase: 'installing' })
            await update.install({ restartAfterInstall: true })
        } catch (error) {
            this.update = null
            this.set({ phase: 'failed', error: String(error) })
            await update.close().catch(() => {})
        }
    }
}

export default new AppUpdateController()

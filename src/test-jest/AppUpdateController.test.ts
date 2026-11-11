jest.mock('@tauri-apps/plugin-updater', () => ({ check: jest.fn() }))

import { AppUpdateController } from 'app/controller/AppUpdateController'
import { Update } from '@tauri-apps/plugin-updater'

const fakeUpdate = () => ({
    version: '2.2.0', body: 'Release notes', close: jest.fn().mockResolvedValue(undefined),
    download: jest.fn().mockResolvedValue(undefined), install: jest.fn().mockResolvedValue(undefined),
})

test('startup checks are quiet offline, manual checks expose retryable errors', async () => {
    const check = jest.fn().mockRejectedValue(new Error('offline'))
    const updates = new AppUpdateController(check)
    await updates.check(true)
    expect(updates.state.visible).toBe(false)
    expect(updates.state.phase).toBe('failed')
    check.mockResolvedValue(null)
    await updates.check()
    expect(updates.state).toMatchObject({ visible: true, phase: 'current', error: undefined })
})

test('checking never installs automatically, duplicate checks reuse the available update', async () => {
    const update = fakeUpdate()
    const check = jest.fn().mockResolvedValue(update)
    const updates = new AppUpdateController(check)
    await updates.check(true)
    expect(updates.state).toMatchObject({ visible: true, phase: 'available', version: '2.2.0' })
    await updates.check()
    expect(check).toHaveBeenCalledTimes(1)
    expect(update.download).not.toHaveBeenCalled()
    expect(update.install).not.toHaveBeenCalled()
    updates.dismiss()
    expect(update.close).toHaveBeenCalledTimes(1)
    await updates.check()
    expect(check).toHaveBeenCalledTimes(2)
})

test('installs only after download succeeds, reports progress and prevents duplicate installs', async () => {
    const update = fakeUpdate()
    let finish!: () => void
    update.download.mockImplementation(callback => {
        callback({ event: 'Started', data: { contentLength: 100 } })
        callback({ event: 'Progress', data: { chunkLength: 40 } })
        callback({ event: 'Progress', data: { chunkLength: 60 } })
        return new Promise<void>(resolve => { finish = resolve })
    })
    const updates = new AppUpdateController(jest.fn().mockResolvedValue(update as unknown as Update))
    await updates.check()
    const installing = updates.install()
    expect(updates.state).toMatchObject({ phase: 'downloading', downloaded: 100, total: 100 })
    updates.dismiss()
    expect(updates.state.visible).toBe(true)
    await updates.install()
    expect(update.download).toHaveBeenCalledTimes(1)
    expect(update.install).not.toHaveBeenCalled()
    finish()
    await installing
    expect(update.install).toHaveBeenCalledWith({ restartAfterInstall: true })
})

test('download/signature failures never launch the installer and allow checking again', async () => {
    const update = fakeUpdate()
    update.download.mockRejectedValue(new Error('signature mismatch'))
    const updates = new AppUpdateController(jest.fn().mockResolvedValue(update))
    await updates.check()
    await updates.install()
    expect(updates.state).toMatchObject({ phase: 'failed', error: 'Error: signature mismatch' })
    expect(update.install).not.toHaveBeenCalled()
    expect(update.close).toHaveBeenCalledTimes(1)
    await updates.check()
    expect(updates.state.phase).toBe('available')
})

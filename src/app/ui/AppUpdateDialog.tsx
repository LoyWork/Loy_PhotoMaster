import React, { useEffect, useState } from 'react'
import { Button, Classes, Dialog, ProgressBar } from '@blueprintjs/core'
import updates from 'app/controller/AppUpdateController'
import { msg } from 'app/i18n/i18n'
import './AppUpdateDialog.less'

export default function AppUpdateDialog() {
    const [state, setState] = useState(updates.state)
    useEffect(() => {
        const unsubscribe = updates.subscribe(() => setState(updates.state))
        // Don't contact release servers from development builds or the UI tester.
        if (__PICTURAMA_PRODUCTION__) void updates.check(true)
        return unsubscribe
    }, [])
    const busy = state.phase === 'downloading' || state.phase === 'installing'
    const progress = state.total ? Math.min(state.downloaded / state.total, 1) : undefined
    return <Dialog isOpen={state.visible} className='AppUpdateDialog bp3-dark' title={msg('Update_title')}
        onClose={updates.dismiss} canEscapeKeyClose={!busy} canOutsideClickClose={!busy} isCloseButtonShown={!busy}>
        <div className={Classes.DIALOG_BODY} aria-live='polite'>
            {state.phase === 'checking' && <p>{msg('Update_checking')}</p>}
            {state.phase === 'current' && <p>{msg('Update_current')}</p>}
            {state.version && <p>{msg('Update_available', state.version)}</p>}
            {state.notes && <pre className='AppUpdateDialog-notes'>{state.notes}</pre>}
            {state.phase === 'available' && <p>{msg('Update_installHint')}</p>}
            {busy && <>
                <p>{msg(state.phase === 'installing' ? 'Update_installing' : 'Update_downloading')}
                    {state.phase === 'downloading' && (state.total
                        ? ` ${Math.round((progress || 0) * 100)}%`
                        : ` ${(state.downloaded / 1024 ** 2).toFixed(1)} MB`)}</p>
                <ProgressBar value={state.phase === 'installing' ? undefined : progress}/>
            </>}
            {state.phase === 'failed' && <div role='alert'><p>{msg('Update_failed')}</p><p>{state.error}</p></div>}
        </div>
        <div className={Classes.DIALOG_FOOTER}><div className={Classes.DIALOG_FOOTER_ACTIONS}>
            <Button disabled={busy} onClick={updates.dismiss}>{msg('Update_close')}</Button>
            {state.phase === 'failed' && <Button onClick={() => void updates.check()}>{msg('Update_check')}</Button>}
            {state.phase === 'available' && <Button intent='primary' icon='download' onClick={() => void updates.install()}>
                {msg('Update_downloadInstall')}</Button>}
        </div></div>
    </Dialog>
}

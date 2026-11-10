import React, { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { Alert, Button, Checkbox, Dialog, HTMLSelect, NonIdealState, ProgressBar, Spinner } from '@blueprintjs/core'
import { convertFileSrc } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import BackgroundClient from 'app/BackgroundClient'
import { PhotoId, SimilarModelStatus, SimilarPhoto, SimilarPhotoGroup, SimilarScanMode, SimilarScanProgress, SimilarScanResult, SimilarSensitivity } from 'app/CommonTypes'
import { fetchPhotoFolders, fetchSections } from 'app/controller/PhotoController'
import { msg } from 'app/i18n/i18n'
import { changePhotosAction } from 'app/state/actions'
import store from 'app/state/store'
import { keepOnlySimilarPhoto, selectSimilarPhoto } from 'app/util/SimilarPhotoSelection'
import './SimilarPhotos.less'
import usePhotoCleanupSession from './usePhotoCleanupSession'

const pageSize = 12
const formatSize = (bytes: number) => bytes >= 1024 ** 3 ? `${(bytes / 1024 ** 3).toFixed(2)} GB` : `${(bytes / 1024 ** 2).toFixed(1)} MB`
const previewSrc = (item: SimilarPhoto) => `${convertFileSrc(item.previewPath)}?v=${encodeURIComponent(item.fingerprint)}`

function PhotoPreview({ item }: { item: SimilarPhoto }) {
    const [failed, setFailed] = useState(false)
    return failed ? <span className='SimilarPhotos-imageError'>{msg('Similar_previewFailed')}</span> :
        <img loading='lazy' src={previewSrc(item)} alt={item.photo.masterFilename} onError={() => setFailed(true)}/>
}

function PhotoGroup({ group, index, selected, disabled, visible, onMore, onToggle, onKeep, onPreview }: {
    group: SimilarPhotoGroup, index: number, selected: Set<PhotoId>, disabled: boolean,
    visible: number, onMore(): void,
    onToggle(id: PhotoId): void, onKeep(id: PhotoId): void, onPreview(item: SimilarPhoto): void,
}) {
    const kept = group.photos.filter(item => !selected.has(item.photo.id)).length
    return <section className='SimilarPhotos-group' aria-label={msg('Similar_group', index, group.photos.length)}>
        <header><h2>{msg('Similar_group', index, group.photos.length)}</h2>
            <span>{msg('Similar_keptCount', kept)}</span></header>
        <div className='SimilarPhotos-contactSheet'>
            {group.photos.slice(0, visible).map(item => {
                const photo = item.photo
                const checked = selected.has(photo.id)
                return <article key={photo.id} aria-label={photo.masterFilename} className={`SimilarPhotos-photo${checked ? ' isDeleting' : ''}`}>
                    <button type='button' className='SimilarPhotos-image' onClick={() => onPreview(item)}
                        aria-label={msg('Similar_previewPhoto', photo.masterFilename)}>
                        <PhotoPreview item={item}/>
                        <span className='SimilarPhotos-previewHint'>{msg('Similar_preview')}</span>
                    </button>
                    <div className='SimilarPhotos-photoInfo'>
                        <strong title={photo.masterFilename}>{photo.flag ? '★ ' : ''}{photo.masterFilename}</strong>
                        <span>{photo.masterWidth} × {photo.masterHeight} · {formatSize(item.fileSize)}</span>
                        <span title={`${photo.masterDir}/${photo.masterFilename}`}>{photo.masterDir}</span>
                    </div>
                    <div className='SimilarPhotos-photoActions'>
                        <Checkbox checked={checked} disabled={disabled || (!checked && kept <= 1)}
                            aria-label={`${msg('Similar_markDelete')} ${photo.masterFilename}`}
                            onChange={() => onToggle(photo.id)} label={msg(checked ? 'Similar_toDelete' : 'Similar_markDelete')}/>
                        <Button small minimal disabled={disabled} onClick={() => onKeep(photo.id)}>{msg('Similar_keepOnly')}</Button>
                    </div>
                </article>
            })}
        </div>
        {visible < group.photos.length && <Button minimal onClick={onMore}>
            {msg('Similar_morePhotos', group.photos.length - visible)}</Button>}
    </section>
}

export default function SimilarPhotos({ kind = 'similar' }: { kind?: 'similar' | 'duplicates' }) {
    const exact = kind === 'duplicates'
    const { session, set } = usePhotoCleanupSession(kind)
    const { mode, model, sensitivity, result, selected, page, notice, hadGroups, expandedGroups } = session
    const setMode = (value: SimilarScanMode) => set('mode', value)
    const setSensitivity = (value: SimilarSensitivity) => set('sensitivity', value)
    const setResult = (value: SimilarScanResult | null) => set('result', value)
    const setSelected = (value: React.SetStateAction<Set<PhotoId>>) => set('selected', value)
    const setPage = (value: number) => set('page', value)
    const setNotice = (value: string) => set('notice', value)
    const setModel = (value: SimilarModelStatus) => set('model', value)
    const [scanning, setScanning] = useState(false)
    const [stopping, setStopping] = useState(false)
    const [progress, setProgress] = useState<SimilarScanProgress | null>(null)
    const [error, setError] = useState('')
    const [confirm, setConfirm] = useState(false)
    const [deleting, setDeleting] = useState(false)
    const [preview, setPreview] = useState<SimilarPhoto | null>(null)
    const mounted = useRef(true)
    const scanId = useRef<string | null>(null)
    const cancelRequested = useRef(false)
    const scroll = useRef<HTMLDivElement>(null)
    const groups = result?.groups || []
    const allPhotos = groups.flatMap(group => group.photos)
    const selectedPhotos = allPhotos.filter(item => selected.has(item.photo.id))
    const selectedSize = selectedPhotos.reduce((sum, item) => sum + item.fileSize, 0)
    const busy = scanning || deleting

    useLayoutEffect(() => {
        const element = scroll.current
        if (!element) return
        element.scrollTop = session.scrollTop
        return () => set('scrollTop', element.scrollTop)
    }, [])

    useEffect(() => {
        if (!exact) BackgroundClient.fetchSimilarModelStatus().then(status => {
            if (!mounted.current) return
            setModel(status)
            if (!status.supported) setMode('quick')
        }).catch(() => {
            if (mounted.current) setModel({ ready: false, supported: true })
        })
        return () => {
            mounted.current = false
            if (scanId.current) BackgroundClient.cancelSimilarScan(scanId.current).catch(console.error)
        }
    }, [])

    async function startScan() {
        if (scanId.current || deleting || (mode === 'deep' && !model?.ready)) return
        const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`
        scanId.current = id
        cancelRequested.current = false
        setScanning(true); setStopping(false); setProgress(null); setError(''); setNotice('')
        setResult(null); setSelected(new Set()); setPage(0); setPreview(null)
        set('hadGroups', false); set('expandedGroups', {})
        scroll.current?.scrollTo(0, 0)
        let unlisten: (() => void) | undefined
        try {
            unlisten = await listen<SimilarScanProgress>('similar-scan-progress', event => {
                if (mounted.current && event.payload.scanId === id) setProgress(event.payload)
            })
            if (!mounted.current) return
            if (cancelRequested.current) { setNotice(msg('Similar_cancelled')); return }
            const next = await BackgroundClient.scanSimilarPhotos(id, sensitivity, mode)
            if (!mounted.current) return
            if (next.cancelled) setNotice(msg('Similar_cancelled'))
            else { setResult(next); set('hadGroups', next.groups.length > 0) }
        } catch (e) {
            const detail = String(e)
            if (mounted.current) setError(exact && detail.includes('Invalid scan mode')
                ? msg('Duplicates_restartRequired')
                : `${msg(exact ? 'Duplicates_scanFailed' : 'Similar_scanFailed')} ${detail}`)
        } finally {
            unlisten?.()
            scanId.current = null
            if (mounted.current) { setScanning(false); setStopping(false) }
        }
    }

    async function cancelScan() {
        if (!scanId.current) return
        cancelRequested.current = true
        setStopping(true)
        try { await BackgroundClient.cancelSimilarScan(scanId.current) }
        catch (e) { setStopping(false); setError(String(e)) }
    }

    async function trashSelected() {
        if (deleting || !selectedPhotos.length) return
        const items = selectedPhotos
        setConfirm(false); setDeleting(true); setError('')
        try {
            await BackgroundClient.updatePhotos(items.map(item => item.photo.id), { trashed: true })
            store.dispatch(changePhotosAction(items.map(item => ({ ...item.photo, trashed: true })), { trashed: true }))
            fetchSections(); fetchPhotoFolders()
            if (!mounted.current) return
            setSelected(new Set()); setPage(0); setPreview(null)
            setNotice(msg('Similar_moved', items.length))
        } catch (e) {
            if (mounted.current) setError(`${msg('Similar_deleteFailed')} ${String(e)}`)
        } finally { if (mounted.current) setDeleting(false) }
    }

    const previewGroup = preview && groups.find(group => group.photos.some(item => item.photo.id === preview.photo.id))
    const previewIndex = previewGroup && preview ? previewGroup.photos.findIndex(item => item.photo.id === preview.photo.id) : -1
    const pageCount = Math.ceil(groups.length / pageSize)
    const currentPage = Math.min(page, Math.max(0, pageCount - 1))
    function changePage(next: number) { setPage(next); scroll.current?.scrollTo(0, 0) }

    return <div className='SimilarPhotos'>
        <header className='SimilarPhotos-heading'>
            <div><h1>{msg(exact ? 'Duplicates_title' : 'Similar_title')}</h1><p>{msg(exact ? 'Duplicates_description' : 'Similar_description')}</p></div>
            <div className='SimilarPhotos-scanControls'>
                {!exact && <><label>{msg('Similar_mode')}
                    <HTMLSelect value={mode} disabled={busy} onChange={e => setMode(e.target.value as SimilarScanMode)} options={[
                        { value: 'quick', label: msg('Similar_quick') },
                        { value: 'deep', label: msg('Similar_deep'), disabled: model?.supported === false },
                    ]}/>
                </label>
                <label>{msg('Similar_sensitivity')}
                    <HTMLSelect value={sensitivity} disabled={busy} onChange={e => setSensitivity(e.target.value as SimilarSensitivity)}
                        options={(['strict', 'standard', 'loose'] as const).map(value => ({ value, label: msg(`Similar_${value}`) }))}/>
                </label></>}
                <Button icon='search' intent='primary' disabled={busy || selected.size > 0 || (mode === 'deep' && !model?.ready)} onClick={startScan}>
                    {msg(result ? 'Similar_rescan' : 'Similar_scan')}</Button>
            </div>
        </header>
        <div className='SimilarPhotos-scroll' ref={scroll}>
            <p className='SimilarPhotos-modeHint'>{msg(exact ? 'Duplicates_scope' : mode === 'deep' ? 'Similar_deepScope' : 'Similar_scope')}</p>
            {mode === 'deep' && !model?.ready && <div className='SimilarPhotos-notice' role='status'>
                {msg(model === null ? 'Similar_checkingModel' : 'Similar_modelMissing')}
            </div>}
            {error && <div className='SimilarPhotos-error' role='alert'>{error}</div>}
            {notice && <div className='SimilarPhotos-notice' role='status'>{notice}</div>}
            {scanning && <div className='SimilarPhotos-scanState' role='status' aria-live='polite'>
                <Spinner size={32}/><h2>{msg(exact ? (progress?.phase === 'hashing' ? 'Duplicates_hashing' :
                    progress?.phase === 'previews' ? 'Duplicates_previews' : 'Duplicates_scanning') :
                    progress?.phase === 'loadingModel' ? 'Similar_loadingModel' :
                    progress?.phase === 'indexing' ? 'Similar_indexing' : progress?.phase === 'grouping' ? 'Similar_grouping' :
                    mode === 'deep' ? 'Similar_deepScanning' : 'Similar_scanning')}</h2>
                <p>{progress && progress.total > 0 ? msg('Similar_progress', progress.processed, progress.total, progress.skipped) : msg('Similar_preparing')}</p>
                <ProgressBar stripes={false} animate={false} value={progress && progress.total > 0 ? progress.processed / progress.total : 0}/>
                <Button disabled={stopping} onClick={cancelScan}>{msg(stopping ? 'Similar_stopping' : 'common_cancel')}</Button>
            </div>}
            {!scanning && !result && <NonIdealState icon='search' title={msg(exact ? 'Duplicates_ready' : 'Similar_ready')}
                description={msg(exact ? 'Duplicates_localOnly' : 'Similar_localOnly')}/>}
            {!scanning && result && <>
                <div className='SimilarPhotos-summary' role='status'>
                    <span>{msg(result.mode === 'exact' ? 'Duplicates_title' : result.mode === 'deep' ? 'Similar_deep' : 'Similar_quick')}</span>
                    {msg('Similar_summary', groups.length, allPhotos.length, result.total)}
                    {result.skipped > 0 && <span>{msg(exact ? 'Duplicates_skipped' : 'Similar_skipped', result.skipped)}</span>}
                </div>
                {groups.length === 0 && <NonIdealState icon='tick' title={msg(hadGroups ? 'Similar_done' : exact ? 'Duplicates_none' : 'Similar_none')}
                    description={msg(exact ? 'Duplicates_localOnly' : 'Similar_localOnly')}/>}
                {groups.slice(currentPage * pageSize, (currentPage + 1) * pageSize).map((group, index) =>
                    <PhotoGroup key={group.photos[0].photo.id} group={group} index={currentPage * pageSize + index + 1}
                        visible={expandedGroups[group.photos[0].photo.id] || 24}
                        onMore={() => set('expandedGroups', current => ({ ...current,
                            [group.photos[0].photo.id]: (current[group.photos[0].photo.id] || 24) + 24 }))}
                        selected={selected} disabled={deleting} onPreview={setPreview}
                        onToggle={id => setSelected(current => selectSimilarPhoto(current, group, id))}
                        onKeep={id => setSelected(current => keepOnlySimilarPhoto(current, group, id))}/>
                )}
                {pageCount > 1 && <nav className='SimilarPhotos-pagination' aria-label={msg('Similar_pages')}>
                    <Button icon='chevron-left' disabled={currentPage === 0} onClick={() => changePage(currentPage - 1)} aria-label={msg('Similar_previousPage')}/>
                    <span>{currentPage + 1} / {pageCount}</span>
                    <Button icon='chevron-right' disabled={currentPage + 1 === pageCount} onClick={() => changePage(currentPage + 1)} aria-label={msg('Similar_nextPage')}/>
                </nav>}
            </>}
        </div>
        <footer className='SimilarPhotos-footer'>
            <div><strong>{msg('Similar_selected', selectedPhotos.length, formatSize(selectedSize))}</strong>
                <span>{msg('Similar_keepHint')}</span></div>
            <div className='SimilarPhotos-footerActions'>
                <Button minimal disabled={busy || !selected.size} onClick={() => setSelected(new Set())}>{msg('Similar_clear')}</Button>
                <Button icon='trash' intent='danger' disabled={busy || !selectedPhotos.length} loading={deleting}
                    onClick={() => setConfirm(true)}>{msg('Similar_trash')}</Button>
            </div>
        </footer>
        <Alert isOpen={confirm} className='bp3-dark' intent='danger' icon='trash'
            confirmButtonText={msg('Similar_trash')} cancelButtonText={msg('common_cancel')}
            onCancel={() => setConfirm(false)} onConfirm={trashSelected}>
            <p>{msg('Similar_confirm', selectedPhotos.length, formatSize(selectedSize))}</p>
        </Alert>
        <Dialog isOpen={!!preview} className='SimilarPhotos-previewDialog bp3-dark'
            title={preview?.photo.masterFilename} onClose={() => setPreview(null)}>
            {preview && previewGroup && <>
                <div className='SimilarPhotos-previewImage'><PhotoPreview key={preview.photo.id} item={preview}/></div>
                <p className='SimilarPhotos-previewPath'>{preview.photo.masterDir}/{preview.photo.masterFilename}</p>
                <div className='SimilarPhotos-previewActions'>
                    <Button icon='chevron-left' aria-label={msg('Similar_previous')} disabled={previewIndex <= 0}
                        onClick={() => setPreview(previewGroup.photos[previewIndex - 1])}/>
                    <span>{previewIndex + 1} / {previewGroup.photos.length}</span>
                    <Button icon='chevron-right' aria-label={msg('Similar_next')} disabled={previewIndex + 1 >= previewGroup.photos.length}
                        onClick={() => setPreview(previewGroup.photos[previewIndex + 1])}/>
                    <Button disabled={deleting} onClick={() => setSelected(current => keepOnlySimilarPhoto(current, previewGroup, preview.photo.id))}>
                        {msg('Similar_keepOnly')}</Button>
                    <Button disabled={deleting || (!selected.has(preview.photo.id) && previewGroup.photos.filter(item => !selected.has(item.photo.id)).length <= 1)}
                        intent={selected.has(preview.photo.id) ? 'danger' : undefined}
                        onClick={() => setSelected(current => selectSimilarPhoto(current, previewGroup, preview.photo.id))}>
                        {msg(selected.has(preview.photo.id) ? 'Similar_cancelDelete' : 'Similar_markDelete')}</Button>
                </div>
            </>}
        </Dialog>
    </div>
}

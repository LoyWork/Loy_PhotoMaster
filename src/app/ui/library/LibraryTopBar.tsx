import classNames from 'classnames'
import React from 'react'
import { Button, MaybeElement, Alert } from '@blueprintjs/core'

import BackgroundClient from 'app/BackgroundClient'
import { EmptyTrashResult } from 'app/CommonTypes'
import { PhotoActionController } from 'app/controller/PhotoActionController'
import { showError } from 'app/ErrorPresenter'
import { msg } from 'app/i18n/i18n'
import { PhotoCollection } from 'app/state/StateTypes'
import PhotoActionButtons from 'app/ui/widget/PhotoActionButtons'
import Toolbar from 'app/ui/widget/Toolbar'
import { bindMany } from 'app/util/LangUtil'

import './LibraryTopBar.less'


interface Props {
    className?: any
    isActive: boolean
    leftItem?: MaybeElement
    selectedPhotos: PhotoCollection | null
    hasSelection: boolean
    isShowingTrash: boolean
    isShowingInfo: boolean
    photosCount: number
    photoActionController: PhotoActionController
    toggleShowInfo(): void
    toggleShowGrading(): void
    onPhotosTrashed(result: EmptyTrashResult): void
}

interface State {
    showEmptyTrashAlert: boolean
}

export default class LibraryTopBar extends React.Component<Props, State> {

    constructor(props: Props) {
        super(props)
        bindMany(this, 'onShowEmptyTrashAlert', 'onEmptyTrashCancelled', 'onEmptyTrashConfirmed')
        this.state = { showEmptyTrashAlert: false }
    }

    private onShowEmptyTrashAlert() {
        this.setState({ showEmptyTrashAlert: true })
    }

    private onEmptyTrashCancelled() {
        this.setState({ showEmptyTrashAlert: false })
    }

    private onEmptyTrashConfirmed() {
        this.setState({ showEmptyTrashAlert: false })
        BackgroundClient.emptyTrash()
            .then(this.props.onPhotosTrashed)
            .catch(error => showError('Trashing photos failed', error))
    }

    render() {
        const { props, state } = this
        return (
            <Toolbar
                className={classNames(props.className, 'LibraryTopBar')}
                isTopBar={true}
                isLeft={true}
                isRight={true}
            >
                {props.leftItem}

                <Toolbar.Spacer isTopBar/>

                {props.isShowingTrash &&
                    <Button
                        className="LibraryTopBar-emptyTrash"
                        icon="trash"
                        text={msg('LibraryTopBar_emptyTrash')}
                        intent={props.photosCount === 0 ? undefined : 'warning'}
                        disabled={props.photosCount === 0}
                        onClick={this.onShowEmptyTrashAlert}
                    />
                }
                <PhotoActionButtons
                    isActive={props.isActive}
                    selectedPhotos={props.selectedPhotos}
                    isShowingTrash={props.isShowingTrash}
                    isShowingInfo={props.isShowingInfo}
                    isShowingColorGrading={false}
                    showPanelButtons={false}
                    rotationsOnly
                    photoActionController={props.photoActionController}
                    toggleShowInfo={props.toggleShowInfo}
                    toggleShowGrading={props.toggleShowGrading}
                />

                {props.hasSelection && props.selectedPhotos && <>
                    <Button minimal icon='star'
                        text={msg('PhotoActionButtons_addFavorite')}
                        onClick={() => props.photoActionController.setPhotosFlagged(props.selectedPhotos!, true)}/>
                    {!props.isShowingTrash && <Button minimal icon='trash'
                        text={msg('PhotoActionButtons_trash')}
                        onClick={() => props.photoActionController.movePhotosToTrash(props.selectedPhotos!)}/>}
                </>}

                <Alert
                    className='LibraryTopBar-emptyTrashAlert'
                    isOpen={state.showEmptyTrashAlert}
                    icon='trash'
                    intent='danger'
                    cancelButtonText={msg('common_cancel')}
                    confirmButtonText={msg('LibraryTopBar_moveToTrash')}
                    onCancel={this.onEmptyTrashCancelled}
                    onConfirm={this.onEmptyTrashConfirmed}
                >
                    {msg('LibraryTopBar_emptyTrashQuestion')}
                </Alert>
            </Toolbar>
        )
    }
}

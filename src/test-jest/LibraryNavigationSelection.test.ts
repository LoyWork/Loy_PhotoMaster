import { PhotoId, PhotoSectionId } from 'app/CommonTypes'
import { emptyTrashAction, fetchSectionsAction, setLibraryActivePhotoAction, setLibraryHoverPhotoAction, setLibrarySelectionAction } from 'app/state/actions'
import { library } from 'app/state/reducers/library'

const sectionId = '2026-11-11' as PhotoSectionId
const photoId = 1 as PhotoId

function selected(all = false) {
    let state = library(undefined, setLibraryActivePhotoAction({ sectionId, photoId }))
    state = library(state, setLibraryHoverPhotoAction({ sectionId, photoId }))
    return library(state, setLibrarySelectionAction({ totalSelectedCount: 2, sectionSelectionById: {
        [sectionId]: { sectionId, selectedCount: 2, selectedPhotosById: all ? 'all' : { [photoId]: true } },
    } }))
}

test.each([
    { filterType: 'folder', folder: 'D:/Photos/Other' } as const,
    { filterType: 'favorites' } as const,
    { filterType: 'trash' } as const,
    { filterType: 'all' } as const,
])('changing to $filterType clears selection and active/hover photos immediately', newFilter => {
    for (const all of [false, true]) {
        const after = library(selected(all), fetchSectionsAction.request({ newFilter }))
        expect(after.selection).toBeNull()
        expect(after.activePhoto).toBeNull()
        expect(after.hoverPhoto).toBeNull()
        // Same date sections in the destination must never resurrect old choices.
        expect(library(after, fetchSectionsAction.success({ sections: [{ id: sectionId, count: 5 }] as any })).selection).toBeNull()
    }
})

test('refreshing the current view keeps selection (e.g. after adding favorites)', () => {
    const before = selected()
    const after = library(before, fetchSectionsAction.request({ newFilter: null }))
    expect(after.selection).toBe(before.selection)
    expect(after.activePhoto).toBe(before.activePhoto)
})

test('emptying trash clears deleted active and hover photos but keeps failed photos available', () => {
    const before = selected()
    const deleted = library(before, emptyTrashAction([photoId]))
    expect(deleted.activePhoto).toBeNull()
    expect(deleted.hoverPhoto).toBeNull()
    expect(deleted.selection).toBeNull()
    const failed = library(before, emptyTrashAction([]))
    expect(failed.activePhoto).toBe(before.activePhoto)
    expect(failed.hoverPhoto).toBe(before.hoverPhoto)
})

export interface Tag {
  name: string
  id: string
  count: number
}

export interface Performer {
  name: string
  id: string
  sceneCount: number
  imageUrl?: string
}

export enum FormStage {
  SelectMode = 1,
  SelectCriteria = 2,
  SelectMarkers = 3,
  VideoOptions = 4,
  Wait = 5,
}

export interface FormState {
  selectMode?: "tags" | "performers"
  selectedIds?: string[]
  clipOrder?: "random" | "scene-order"
  clipDuration?: number
  outputResolution?: "720" | "1080" | "4K"
  outputFps?: number
  selectedMarkers?: string[]
  markers?: unknown[]
  stage: FormStage
  id: string
}

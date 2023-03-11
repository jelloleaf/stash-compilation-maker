import clsx from "clsx"
import {useStateMachine} from "little-state-machine"
import {useEffect} from "react"
import {Outlet, useLocation, useNavigate} from "react-router-dom"
import {FormStage} from "../types/types"
import {resetForm} from "./actions"

const stageMap = {
  [FormStage.SelectMode]: "/",
  [FormStage.SelectCriteria]: "/select-criteria",
  [FormStage.SelectMarkers]: "/select-markers",
  [FormStage.VideoOptions]: "/video-options",
  [FormStage.Wait]: "/progress",
}

export default function Root() {
  const {state, actions} = useStateMachine({resetForm})
  const stage = state.data.stage
  const correctUrl = stageMap[stage]
  const location = useLocation()
  const navigate = useNavigate()

  useEffect(() => {
    if (location.pathname !== correctUrl) {
      navigate(correctUrl)
    }
  }, [, correctUrl, location])

  const onReset = () => {
    if (
      confirm(
        "Are you sure you want to reset the form and return to the start?"
      )
    ) {
      actions.resetForm()
      navigate("/")
    }
  }

  return (
    <div className="min-h-screen flex flex-col justify-between">
      <main className="container ml-auto mr-auto">
        <section className="py-4 flex flex-col">
          <h1 className="text-4xl font-bold mb-4 text-center">
            Stash Compilation Generator
          </h1>
          <div className="self-center flex gap-2 mb-4">
            <button onClick={onReset} className="btn btn-sm btn-error">
              Reset
            </button>
          </div>
          <ul className="steps mb-4">
            <li className="step step-primary">Choose mode</li>
            <li
              className={clsx(
                "step",
                stage >= FormStage.SelectCriteria && "step-primary"
              )}
            >
              Select criteria
            </li>
            <li
              className={clsx(
                "step",
                stage >= FormStage.SelectMarkers && "step-primary"
              )}
            >
              Select markers
            </li>
            <li
              className={clsx(
                "step",
                stage >= FormStage.VideoOptions && "step-primary"
              )}
            >
              Select video options
            </li>
            <li
              className={clsx(
                "step",
                stage >= FormStage.Wait && "step-primary"
              )}
            >
              Wait for video
            </li>
          </ul>
          <Outlet />
        </section>
      </main>
      <footer className="w-full text-center text-sm font-light flex flex-col gap-1 my-4">
        <p>
          Made by{" "}
          <a href="https://soundchaser128.xyz" className="link">
            soundchaser128
          </a>
        </p>
        <p>
          This project is open source and available on{" "}
          <a
            className="link"
            href="https://github.com/soundchaser128/stash-compilation-maker"
          >
            GitHub
          </a>
        </p>
      </footer>
    </div>
  )
}

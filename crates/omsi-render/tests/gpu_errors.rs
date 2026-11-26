//! Tests moved out of `src/gpu/errors.rs`.

use omsi_render::testing::internals::gpu::errors::*;
use std::sync::atomic::Ordering;

fn validation() -> wgpu::Error {
    wgpu::Error::Validation {
        source: Box::new(std::io::Error::other("bad")),
        description: "a refused call\nover two lines ".into(),
    }
}

#[test]
fn a_validation_errors_text_is_its_description_on_one_line() {
    assert_eq!(
        gpu_error_text(&validation()),
        "a refused call over two lines"
    );
    assert_eq!(gpu_error_text(&fake_out_of_memory()), "Out of Memory");
}

#[test]
fn running_out_of_memory_is_noted_for_the_textures() {
    let errors = GpuErrors::new(1);
    assert_eq!(errors.on_error(&fake_out_of_memory()), ErrorOutcome::Logged);
    assert!(errors.out_of_memory.load(Ordering::Relaxed));
    // (an error of another kind does not say so)
    let errors = GpuErrors::new(1);
    errors.on_error(&validation());
    assert!(!errors.out_of_memory.load(Ordering::Relaxed));
}

#[test]
fn the_oom_hook_reports_once_through_the_handler() {
    let errors = GpuErrors::new(1);
    assert!(errors.fake_out_of_memory_once());
    assert!(errors.out_of_memory.swap(false, Ordering::Relaxed));
    assert!(!errors.fake_out_of_memory_once());
    assert!(!errors.out_of_memory.load(Ordering::Relaxed));
}

#[test]
fn with_msaa_the_first_error_switches_it_off_then_errors_are_logged() {
    let errors = GpuErrors::new(4);
    assert_eq!(errors.on_error(&validation()), ErrorOutcome::MsaaOff);
    assert!(errors.gpu_error.load(Ordering::Relaxed));
    assert_eq!(errors.on_error(&validation()), ErrorOutcome::Logged);
}

#[test]
fn without_msaa_the_first_twenty_and_every_thousandth_are_logged() {
    let errors = GpuErrors::new(1);
    let outcomes: Vec<ErrorOutcome> = (0..1001).map(|_| errors.on_error(&validation())).collect();
    assert!(outcomes[..20].iter().all(|o| *o == ErrorOutcome::Logged));
    assert!(outcomes[20..1000].iter().all(|o| *o == ErrorOutcome::Quiet));
    assert_eq!(outcomes[1000], ErrorOutcome::Logged);
    assert!(!errors.gpu_error.load(Ordering::Relaxed));
}

#[test]
fn a_panic_caught_gives_none_and_restores_the_catching_state() {
    assert!(!catching());
    assert_eq!(catch(|| 7), Some(7));
    assert_eq!(catch(|| -> u8 { panic!("a driver went down") }), None);
    assert!(!catching());
    // (a catch inside a catch leaves the outer one catching)
    let inner = catch(|| {
        let r = catch(|| -> u8 { panic!("inner") });
        (r, catching())
    });
    assert_eq!(inner, Some((None, true)));
    assert!(!catching());
}

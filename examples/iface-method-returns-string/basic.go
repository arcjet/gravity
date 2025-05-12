package hello

import "context"
import "errors"
import "github.com/tetratelabs/wazero"
import "github.com/tetratelabs/wazero/api"

import _ "embed"

//go:embed hello.wasm
var wasmFileHello []byte

type HelloFactory struct {
	runtime wazero.Runtime
	module wazero.CompiledModule
}

type IHelloRuntime interface {
	Os(
		ctx context.Context,
	) string
	Arch(
		ctx context.Context,
	) string
	Puts(
		ctx context.Context,
		msg string,
	)
}

func NewHelloFactory(
	ctx context.Context,
	runtime IHelloRuntime,
) (*HelloFactory, error) {
	wasmRT := wazero.NewRuntime(ctx)

	_, err0 := wasmRT.NewHostModuleBuilder("arcjet:basic/runtime").
	NewFunctionBuilder().
	WithFunc(func(
		ctx context.Context,
		mod api.Module,
		arg0 uint32,
	) {
		value0 := runtime.Os(ctx, )
		memory1 := mod.Memory()
		realloc1 := mod.ExportedFunction("cabi_realloc")
		ptr1, len1, err1 := writeString(ctx, value0, memory1, realloc1)
		if err1 != nil {
			panic(err1)
		}
		mod.Memory().WriteUint32Le(arg0+4, uint32(len1))
		mod.Memory().WriteUint32Le(arg0+0, uint32(ptr1))
	}).
	Export("os").
	NewFunctionBuilder().
	WithFunc(func(
		ctx context.Context,
		mod api.Module,
		arg0 uint32,
	) {
		value0 := runtime.Arch(ctx, )
		memory1 := mod.Memory()
		realloc1 := mod.ExportedFunction("cabi_realloc")
		ptr1, len1, err1 := writeString(ctx, value0, memory1, realloc1)
		if err1 != nil {
			panic(err1)
		}
		mod.Memory().WriteUint32Le(arg0+4, uint32(len1))
		mod.Memory().WriteUint32Le(arg0+0, uint32(ptr1))
	}).
	Export("arch").
	NewFunctionBuilder().
	WithFunc(func(
		ctx context.Context,
		mod api.Module,
		arg0 uint32,
		arg1 uint32,
	) {
		buf0, ok0 := mod.Memory().Read(arg0, arg1)
		if !ok0 {
			panic(errors.New("failed to read bytes from memory"))
		}
		str0 := string(buf0)
		runtime.Puts(ctx, str0)
	}).
	Export("puts").
	Instantiate(ctx)
	if err0 != nil {
		return nil, err0
	}

	// Compiling the module takes a LONG time, so we want to do it once and hold
	// onto it with the Runtime
	module, err := wasmRT.CompileModule(ctx, wasmFileHello)
	if err != nil {
		return nil, err
	}

	return &HelloFactory{wasmRT, module}, nil
}

func (f *HelloFactory) Instantiate(ctx context.Context) (*HelloInstance, error) {
	if module, err := f.runtime.InstantiateModule(ctx, f.module, wazero.NewModuleConfig()); err != nil {
		return nil, err
	} else {
		return &HelloInstance{module}, nil
	}
}

func (f *HelloFactory) Close(ctx context.Context) {
	f.runtime.Close(ctx)
}

type HelloInstance struct {
	module api.Module
}

// writeString will put a Go string into the Wasm memory following the Component
// Model calling convetions, such as allocating memory with the realloc function
func writeString(
	ctx context.Context,
	s string,
	memory api.Memory,
	realloc api.Function,
) (uint64, uint64, error) {
	if len(s) == 0 {
		return 1, 0, nil
	}

	results, err := realloc.Call(ctx, 0, 0, 1, uint64(len(s)))
	if err != nil {
		return 1, 0, err
	}
	ptr := results[0]
	ok := memory.Write(uint32(ptr), []byte(s))
	if !ok {
		return 1, 0, err
	}
	return uint64(ptr), uint64(len(s)), nil
}

func (i *HelloInstance) Close(ctx context.Context) error {
	if err := i.module.Close(ctx); err != nil {
		return err
	}

	return nil
}

func (i *HelloInstance) Hello(
	ctx context.Context,
) (string, error) {
	raw0, err0 := i.module.ExportedFunction("hello").Call(ctx, )
	if err0 != nil {
		var default0 string
		return default0, err0
	}

	// The cleanup via `cabi_post_*` cleans up the memory in the guest. By
	// deferring this, we ensure that no memory is corrupted before the function
	// is done accessing it.
	defer func() {
		if _, err := i.module.ExportedFunction("cabi_post_hello").Call(ctx, raw0...); err != nil {
			// If we get an error during cleanup, something really bad is
			// going on, so we panic. Also, you can't return the error from
			// the `defer`
			panic(errors.New("failed to cleanup"))
		}
	}()

	results0 := raw0[0]
	value1, ok1 := i.module.Memory().ReadByte(uint32(results0 + 0))
	if !ok1 {
		var default1 string
		return default1, errors.New("failed to read byte from memory")
	}
	var value8 string
	var err8 error
	switch value1 {
	case 0:
		ptr2, ok2 := i.module.Memory().ReadUint32Le(uint32(results0 + 4))
		if !ok2 {
			var default2 string
			return default2, errors.New("failed to read pointer from memory")
		}
		len3, ok3 := i.module.Memory().ReadUint32Le(uint32(results0 + 8))
		if !ok3 {
			var default3 string
			return default3, errors.New("failed to read length from memory")
		}
		buf4, ok4 := i.module.Memory().Read(ptr2, len3)
		if !ok4 {
			var default4 string
			return default4, errors.New("failed to read bytes from memory")
		}
		str4 := string(buf4)
		value8 = str4
	case 1:
		ptr5, ok5 := i.module.Memory().ReadUint32Le(uint32(results0 + 4))
		if !ok5 {
			var default5 string
			return default5, errors.New("failed to read pointer from memory")
		}
		len6, ok6 := i.module.Memory().ReadUint32Le(uint32(results0 + 8))
		if !ok6 {
			var default6 string
			return default6, errors.New("failed to read length from memory")
		}
		buf7, ok7 := i.module.Memory().Read(ptr5, len6)
		if !ok7 {
			var default7 string
			return default7, errors.New("failed to read bytes from memory")
		}
		str7 := string(buf7)
		err8 = errors.New(str7)
	default:
		err8 = errors.New("invalid variant discriminant for expected")
	}
	return value8, err8
}

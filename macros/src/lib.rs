use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};

#[proc_macro_derive(AsBits)]
pub fn as_bits(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let enum_name = input.ident;
    let Data::Enum(data_enum) = input.data else {
        panic!("Cannot apply `AsBits` to a non-enum");
    };

    let repr_size: Type = input
        .attrs
        .iter()
        .find_map(|attr| match &attr.meta {
            syn::Meta::List(list) => list.parse_args().ok().filter(|_| {
                let segments = &list.path.segments;
                segments.len() == 1
                    && segments.first().expect("Length should have been one").ident == "repr"
            }),
            _ => None,
        })
        .expect("Enum should specify a primitive representation");

    let arms: Box<_> = data_enum
        .variants
        .iter()
        .map(|variant| {
            match variant.fields {
                Fields::Unit => {}
                _ => panic!("Cannot apply `AsBits` to an enum with a non-unit variant"),
            }

            let variant_name = &variant.ident;
            let discriminant = &variant
                .discriminant
                .as_ref()
                .expect("All enum variants should specify their discriminant")
                .1;

            quote! {
                #discriminant => Self::#variant_name,
            }
        })
        .collect();

    quote! {
        impl #enum_name {
            pub const fn into_bits(self) -> #repr_size {
                self as _
            }

            pub const fn from_bits(value: #repr_size) -> Self {
                match value {
                    #(#arms)*
                    _ => panic!("Unexpected value for enum")
                }
            }
        }
    }
    .into()
}

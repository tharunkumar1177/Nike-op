using System.ComponentModel;
using EdgeOptimizer.Settings.Core.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Storage;

namespace EdgeOptimizer.Settings.WinUI.Views;

public partial class CrosshairView : UserControl
{
    private CrosshairViewModel? _viewModel;
    private bool _imageLoadFailed;

    public CrosshairView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
        Unloaded += (_, _) =>
        {
            if (_viewModel is not null) _viewModel.PropertyChanged -= ViewModelPropertyChanged;
        };
    }

    private void OnDataContextChanged(FrameworkElement sender, DataContextChangedEventArgs args)
    {
        if (_viewModel is not null) _viewModel.PropertyChanged -= ViewModelPropertyChanged;
        _viewModel = args.NewValue as CrosshairViewModel;
        if (_viewModel is not null) _viewModel.PropertyChanged += ViewModelPropertyChanged;
        _ = LoadPreviewAsync();
    }

    private void ViewModelPropertyChanged(object? sender, PropertyChangedEventArgs args)
    {
        if (args.PropertyName == nameof(CrosshairViewModel.ImagePath)) _ = LoadPreviewAsync();
        else if (args.PropertyName == nameof(CrosshairViewModel.OverlayEnabled)) UpdateNotice();
    }

    private async Task LoadPreviewAsync()
    {
        var path = _viewModel?.ImagePath;
        _imageLoadFailed = false;
        if (string.IsNullOrWhiteSpace(path))
        {
            ShowFallback();
            return;
        }

        try
        {
            var file = await StorageFile.GetFileFromPathAsync(path);
            await using var stream = await file.OpenStreamForReadAsync();
            var bitmap = new BitmapImage();
            await bitmap.SetSourceAsync(stream.AsRandomAccessStream());
            if (_viewModel?.ImagePath != path) return;
            PreviewImage.Source = bitmap;
            FallbackCrosshair.Visibility = Visibility.Collapsed;
            UpdateNotice();
        }
        catch
        {
            if (_viewModel?.ImagePath != path) return;
            _imageLoadFailed = true;
            ShowFallback();
        }
    }

    private void ShowFallback()
    {
        PreviewImage.Source = null;
        FallbackCrosshair.Visibility = Visibility.Visible;
        UpdateNotice();
    }

    private void UpdateNotice()
    {
        string? notice = _viewModel switch
        {
            null => null,
            { OverlayEnabled: false } => "The overlay is off for this profile.",
            _ when _imageLoadFailed => "This image could not be loaded. Choose another PNG.",
            { HasImage: false } => "No image selected. The overlay will not start until you choose a PNG.",
            _ => null,
        };
        PreviewNoticeText.Text = notice ?? string.Empty;
        PreviewNotice.Visibility = notice is null ? Visibility.Collapsed : Visibility.Visible;
    }
}

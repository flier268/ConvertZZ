using System.Collections.Generic;
using System.Windows;
using System.Windows.Controls;
using ConvertZZ.ViewModels;

namespace ConvertZZ.Views
{
    /// <summary>
    /// CheckboxTreeView.xaml 的互動邏輯
    /// </summary>
    public partial class CheckboxTreeView : UserControl
    {
        public CheckboxTreeView()
        {
            InitializeComponent();
        }

        public IList<Node> ItemSources
        {
            get { return (IList<Node>)GetValue(ItemSourcesProperty); }
            set { SetValue(ItemSourcesProperty, value); }
        }

        public static readonly DependencyProperty ItemSourcesProperty =
            DependencyProperty.Register(nameof(ItemSources), typeof(IList<Node>), typeof(CheckboxTreeView), new UIPropertyMetadata(null));
    }
}